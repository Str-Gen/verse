//! Disk-backed normalized books. Only a bounded playback window is resident.
use crate::{Block, Book, Document, Error, Note, Piece, Resource, Result, SourceSpan, session};
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
mod cached;
mod records;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const WINDOW_PIECES: usize = 512;
const MAX_RECORD: u64 = 64 * 1024 * 1024;
const MAX_PIECE_BYTES: usize = 16 * 1024;
const MAX_CONTEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NOTE_BYTES: u64 = 1024 * 1024;
const MAX_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct BookWindow {
    pub book: Book,
    pub start: usize,
    pub total: usize,
    pub image_dimensions: BTreeMap<String, [u32; 2]>,
}

#[derive(Debug)]
pub struct DiskBook {
    root: PathBuf,
    _lease: File,
    persistent: bool,
    metadata: Book,
    total: usize,
    chapter_starts: Vec<usize>,
    records: records::Records,
    reader: Mutex<PieceReader>,
}

#[derive(Debug)]
struct PieceReader {
    index: File,
    pieces: BufReader<File>,
    buffer: Vec<u8>,
}

impl PieceReader {
    fn seek_index(&mut self, index: usize) -> Result<()> {
        let offset = (index as u64)
            .checked_mul(16)
            .ok_or(Error::Limit("piece index"))?;
        self.index.seek(SeekFrom::Start(offset))?;
        Ok(())
    }

    fn read_piece(&mut self, entry: &[u8; 16], previous_end: Option<u64>) -> Result<(Piece, u64)> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&entry[..8]);
        let offset = u64::from_le_bytes(bytes);
        bytes.copy_from_slice(&entry[8..]);
        let length = u64::from_le_bytes(bytes);
        if length > MAX_PIECE_BYTES as u64 {
            return Err(Error::Limit("disk piece record"));
        }
        let end = offset
            .checked_add(length)
            .ok_or(Error::Limit("piece offset"))?;
        match previous_end {
            Some(previous) if previous == offset => {}
            Some(previous) if previous.checked_add(1) == Some(offset) => {
                // Spool records are separated by one newline; retain buffered following records.
                self.pieces.seek_relative(1)?;
            }
            _ => {
                self.pieces.seek(SeekFrom::Start(offset))?;
            }
        }
        self.buffer.resize(length as usize, 0);
        if let Err(error) = self.pieces.read_exact(&mut self.buffer) {
            return if error.kind() == std::io::ErrorKind::UnexpectedEof {
                Err(invalid("truncated disk book"))
            } else {
                Err(error.into())
            };
        }
        Ok((serde_json::from_slice(&self.buffer).map_err(invalid)?, end))
    }
}

fn invalid(error: impl fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}
#[cfg(test)]
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    serde_json::to_writer(&mut file, value).map_err(invalid)?;
    file.flush()?;
    Ok(())
}

fn cache_directory() -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or_else(|| invalid("no local cache directory configured"))?;
    Ok(root.join("quiet-reader").join("book-spools"))
}
fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn create_spool(base: &Path) -> Result<(PathBuf, File)> {
    private_directory(base)?;
    let guard = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(base.join("spools.lock"))?;
    guard.lock()?;
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name
            .strip_prefix("quiet-reader-book-")
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            continue;
        }
        let Ok(lease) = File::open(entry.path().join("lease")) else {
            continue;
        };
        if lease.try_lock().is_ok() {
            // The creation lock prevents racing another opener before it acquires its lease.
            fs::remove_dir_all(entry.path())?;
        }
    }
    let root = base.join(format!("quiet-reader-book-{}", uuid::Uuid::new_v4()));
    private_directory(&root)?;
    let lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(root.join("lease"))?;
    lease.lock()?;
    Ok((root, lease))
}

impl DiskBook {
    /// Streams an export into a private disk spool. Spools are removed on drop.
    /// No book-sized piece index or decoded image collection is retained in memory.
    pub fn open(path: &Path) -> Result<Self> {
        let (root, lease) = create_spool(&cache_directory()?)?;
        let result = Self::build(path, root.clone(), lease);
        if result.is_err() {
            let _ = fs::remove_dir_all(root);
        }
        result
    }
    fn build(path: &Path, root: PathBuf, lease: File) -> Result<Self> {
        let mut sink = Sink {
            records: records::Writer::new(&root)?,
            block_kinds: BTreeMap::new(),
            starts: BTreeMap::new(),
            late_chapters: false,
            pieces: BufWriter::new(File::create(root.join("pieces"))?),
            index: BufWriter::new(File::create(root.join("index"))?),
            images: BufWriter::new(File::create(root.join("images"))?),
            offset: 0,
            total: 0,
        };
        let mut decoder = serde_json::Deserializer::from_reader(BufReader::new(File::open(path)?));
        let metadata = BookSeed(&mut sink)
            .deserialize(&mut decoder)
            .map_err(invalid)?;
        decoder.end().map_err(invalid)?;
        if serde_json::to_vec(&metadata).map_err(invalid)?.len() > MAX_METADATA_BYTES {
            return Err(Error::Limit("book metadata bytes"));
        }
        sink.pieces.flush()?;
        sink.index.flush()?;
        sink.images.flush()?;
        let total = sink.total;
        let mut starts = std::mem::take(&mut sink.starts);
        let late_chapters = sink.late_chapters;
        sink.records.finish(&root)?;
        let reader = Mutex::new(PieceReader {
            index: File::open(root.join("index"))?,
            pieces: BufReader::new(File::open(root.join("pieces"))?),
            buffer: Vec::new(),
        });
        let mut result = Self {
            reader,
            records: records::Records::open(&root)?,
            root,
            _lease: lease,
            persistent: false,
            metadata,
            total,
            chapter_starts: Vec::new(),
        };
        if late_chapters {
            let mut reader = BufReader::new(File::open(result.root.join("pieces"))?);
            let mut line = Vec::new();
            let mut index = 0;
            while std::io::BufRead::read_until(&mut reader, b'\n', &mut line)? != 0 {
                #[derive(Deserialize)]
                struct Identity {
                    id: String,
                }
                let piece: Identity = serde_json::from_slice(&line).map_err(invalid)?;
                if let Some(start) = starts.get_mut(&piece.id)
                    && start.replace(index).is_some()
                {
                    return Err(invalid("duplicate chapter piece"));
                }
                line.clear();
                index += 1;
            }
        }
        for chapter in &result.metadata.chapters {
            result.chapter_starts.push(
                starts
                    .get(&chapter.start_piece)
                    .copied()
                    .flatten()
                    .ok_or_else(|| invalid("chapter references missing piece"))?,
            );
        }
        Ok(result)
    }
    pub fn metadata(&self) -> &Book {
        &self.metadata
    }
    pub fn total_pieces(&self) -> usize {
        self.total
    }
    pub fn chapter_starts(&self) -> &[usize] {
        &self.chapter_starts
    }
    fn named<T: de::DeserializeOwned>(&self, group: &str, id: &str) -> Result<T> {
        self.records
            .get(group, id, MAX_RECORD)?
            .ok_or_else(|| invalid("missing disk record"))
    }
    pub fn piece(&self, index: usize) -> Result<Piece> {
        if index >= self.total {
            return Err(invalid("piece index outside book"));
        }
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| invalid("book reader poisoned"))?;
        reader.seek_index(index)?;
        let mut entry = [0; 16];
        reader.index.read_exact(&mut entry)?;
        reader.read_piece(&entry, None).map(|(piece, _)| piece)
    }

    pub fn find_piece(&self, id: &str) -> Result<Option<usize>> {
        self.find(|piece| piece.id == id)
    }
    fn find(&self, predicate: impl Fn(&Piece) -> bool) -> Result<Option<usize>> {
        let mut result = None;
        let reader = BufReader::new(File::open(self.root.join("pieces"))?);
        for (index, piece) in serde_json::Deserializer::from_reader(reader)
            .into_iter::<Piece>()
            .enumerate()
        {
            if predicate(&piece.map_err(invalid)?) {
                if result.is_some() {
                    return Ok(None);
                }
                result = Some(index);
            }
        }
        Ok(result)
    }
    pub fn find_source(&self, sources: &[SourceSpan]) -> Result<Option<usize>> {
        if sources.is_empty() {
            return Ok(None);
        }
        self.find(|piece| piece.sources == sources)
    }
    pub fn restore(&self, saved: Option<&Value>) -> Result<session::Resume> {
        let Some(saved) = saved else {
            return Ok(session::Resume {
                kind: "fresh".into(),
                at: 0,
            });
        };
        let location = saved.get("location").unwrap_or(saved);
        if location
            .get("book_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id != self.metadata.book_id)
        {
            return Ok(session::Resume {
                kind: "unresolved".into(),
                at: 0,
            });
        }
        let revision = saved
            .get("revision")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("saved record has no valid revision"))?;
        if revision == self.metadata.revision
            && let Some(sources) = location.get("sources")
        {
            let sources: Vec<SourceSpan> =
                serde_json::from_value(sources.clone()).map_err(invalid)?;
            let text = location.get("text").and_then(Value::as_str);
            if let Some(at) = saved
                .get("piece_index")
                .and_then(Value::as_u64)
                .and_then(|at| usize::try_from(at).ok())
                .filter(|at| *at < self.total)
            {
                let piece = self.piece(at)?;
                let source_matches = !sources.is_empty() && piece.sources == sources;
                let event = saved.get("event_anchor");
                let event_matches = sources.is_empty()
                    && piece.sources.is_empty()
                    && piece.kind != "word"
                    && location.get("book_id").and_then(Value::as_str)
                        == Some(self.metadata.book_id.as_str())
                    && location.get("revision").and_then(Value::as_str)
                        == Some(self.metadata.revision.as_str())
                    && event
                        .and_then(|event| event.get("piece_id"))
                        .and_then(Value::as_str)
                        == Some(piece.id.as_str())
                    && event
                        .and_then(|event| event.get("block_id"))
                        .and_then(Value::as_str)
                        == Some(piece.block_id.as_str())
                    && event
                        .and_then(|event| event.get("kind"))
                        .and_then(Value::as_str)
                        == Some(piece.kind.as_str());
                if (source_matches || event_matches) && text == Some(piece.text.as_str()) {
                    return Ok(session::Resume {
                        kind: "exact".into(),
                        at,
                    });
                }
            }
            if let Some(at) = self.find(|p| {
                !sources.is_empty() && p.sources == sources && text.is_none_or(|t| p.text == t)
            })? {
                return Ok(session::Resume {
                    kind: "exact".into(),
                    at,
                });
            }
            if let Some(first) = sources.first()
                && let Some(at) = self.find(|p| {
                    p.sources.iter().any(|s| {
                        s.document == first.document
                            && s.node == first.node
                            && s.offset <= first.offset
                            && first.offset < s.offset.saturating_add(s.length)
                    })
                })?
            {
                let window = self.load_window(at)?;
                let resume = session::restore(&window.book, Some(saved)).map_err(invalid)?;
                if resume.kind == "exact" {
                    return Ok(session::Resume {
                        kind: resume.kind,
                        at: window.start + resume.at,
                    });
                }
            }
        }
        let chapter = location
            .get("chapter_id")
            .and_then(Value::as_str)
            .or_else(|| {
                saved
                    .get("chapter")
                    .and_then(|c| c.as_str().or_else(|| c.get("id").and_then(Value::as_str)))
            });
        let title = location
            .get("chapter_title")
            .and_then(Value::as_str)
            .or_else(|| {
                saved
                    .get("chapter")
                    .and_then(|chapter| chapter.get("title"))
                    .and_then(Value::as_str)
            });
        let index = self.metadata.chapters.iter().position(|c| {
            Some(c.id.as_str()) == chapter
                && (revision == self.metadata.revision || Some(c.title.as_str()) == title)
        });
        Ok(session::Resume {
            kind: if index.is_some() {
                "chapter"
            } else {
                "unresolved"
            }
            .into(),
            at: index.map_or(0, |i| self.chapter_starts[i]),
        })
    }
    pub fn capture(&self, at: usize, preferences: &Value) -> Result<Value> {
        if at >= self.total {
            return Err(invalid("piece index outside book"));
        }
        let start = at.saturating_sub(8);
        let end = at.saturating_add(16).min(self.total);
        let mut book = self.metadata.clone();
        for index in start..end {
            book.pieces.push(self.piece(index)?);
        }
        book.chapters.clear();
        for (index, chapter) in self.metadata.chapters.iter().enumerate() {
            let chapter_start = self.chapter_starts[index];
            let chapter_end = self
                .chapter_starts
                .get(index + 1)
                .copied()
                .unwrap_or(self.total);
            if chapter_start < end && chapter_end > start {
                let mut chapter = chapter.clone();
                if chapter_start < start
                    && let Some(first) = book.pieces.first()
                {
                    chapter.start_piece = first.id.clone();
                }
                if chapter_end >= end {
                    chapter.end_piece_exclusive = None;
                }
                book.chapters.push(chapter);
            }
        }
        let piece = &book.pieces[at - start];
        let mut record = if piece.sources.is_empty()
            && piece.kind != "word"
            && !piece.id.is_empty()
            && !piece.block_id.is_empty()
        {
            let location =
                crate::location::SavedLocation::capture(&book, &piece.id).map_err(invalid)?;
            let mut record = preferences.as_object().cloned().unwrap_or_default();
            record.insert("revision".into(), serde_json::json!(book.revision));
            record.insert("sources".into(), serde_json::json!([]));
            record.insert(
                "location".into(),
                serde_json::to_value(location).map_err(invalid)?,
            );
            record.insert("event_anchor".into(),serde_json::json!({"piece_id":piece.id,"block_id":piece.block_id,"kind":piece.kind}));
            let speed = record
                .get("speed")
                .and_then(Value::as_u64)
                .and_then(|speed| u32::try_from(speed).ok())
                .unwrap_or(350);
            record.insert("speed".into(), serde_json::json!(session::speed(speed)));
            record.remove("context");
            record.remove("chapter");
            Value::Object(record)
        } else {
            let mut record = session::capture(&book, at - start, preferences).map_err(invalid)?;
            if let Some(object) = record.as_object_mut() {
                object.remove("event_anchor");
            }
            record
        };
        record["piece_index"] = serde_json::json!(at);
        Ok(record)
    }
    pub fn load_window(&self, at: usize) -> Result<BookWindow> {
        if self.total > 0 && at >= self.total {
            return Err(invalid("piece index outside book"));
        }
        let (chapter_start, chapter_end) = self.chapter_bounds(at);
        let start = chapter_start + (at - chapter_start) / WINDOW_PIECES * WINDOW_PIECES;
        self.load_range(start, start.saturating_add(WINDOW_PIECES).min(chapter_end))
    }

    pub fn load_chapter(&self, at: usize) -> Result<BookWindow> {
        if self.total > 0 && at >= self.total {
            return Err(invalid("piece index outside book"));
        }
        let (start, end) = self.chapter_bounds(at);
        self.load_range(start, end)
    }

    fn chapter_bounds(&self, at: usize) -> (usize, usize) {
        let chapter_start = self
            .chapter_starts
            .iter()
            .copied()
            .filter(|&start| start <= at)
            .max()
            .unwrap_or(0);
        let chapter_end = self
            .chapter_starts
            .iter()
            .copied()
            .filter(|&start| start > at)
            .min()
            .unwrap_or(self.total);
        (chapter_start, chapter_end)
    }

    fn load_range(&self, start: usize, end: usize) -> Result<BookWindow> {
        let mut book = self.metadata.clone();
        if start < end {
            let mut reader = self
                .reader
                .lock()
                .map_err(|_| invalid("book reader poisoned"))?;
            reader.seek_index(start)?;
            let mut entries = [[0; 16]; WINDOW_PIECES];
            let mut previous_end = None;
            let mut remaining = end - start;
            let mut bytes = 0u64;
            while remaining > 0 {
                let count = remaining.min(WINDOW_PIECES);
                let entries = &mut entries[..count];
                reader.index.read_exact(entries.as_flattened_mut())?;
                for entry in entries {
                    bytes = bytes.saturating_add(u64::from_le_bytes(
                        entry[8..].try_into().map_err(invalid)?,
                    ));
                    if bytes > MAX_RECORD {
                        return Err(Error::Limit("chapter piece bytes"));
                    }
                    let (piece, end) = reader.read_piece(entry, previous_end)?;
                    book.pieces.push(piece);
                    previous_end = Some(end);
                }
                remaining -= count;
            }
        }
        let mut pieces_by_block: BTreeMap<&str, Vec<&Piece>> = BTreeMap::new();
        let mut block_order = BTreeMap::new();
        for (index, piece) in book.pieces.iter().enumerate() {
            pieces_by_block
                .entry(&piece.block_id)
                .or_default()
                .push(piece);
            block_order.entry(piece.block_id.as_str()).or_insert(index);
        }
        let mut documents: BTreeMap<String, Vec<Block>> = BTreeMap::new();
        let mut context_bytes = 0u64;
        let mut boundary_blocks = None;
        let mut image_dimensions = BTreeMap::new();
        for (&id, pieces) in &pieces_by_block {
            let Some(mut block) = self.records.get::<Block>("blocks", id, MAX_CONTEXT_BYTES)?
            else {
                continue;
            };
            if let Some(resource) = &block.resource
                && let Some(dimensions) = self.records.get("dimensions", resource, MAX_RECORD)?
            {
                image_dimensions.insert(resource.clone(), dimensions);
            }
            let original = std::mem::take(&mut block.runs);
            let (previous, next) = match &boundary_blocks {
                Some(blocks) => blocks,
                None => boundary_blocks.insert((
                    if start > 0 {
                        Some(self.piece(start - 1)?.block_id)
                    } else {
                        None
                    },
                    if end < self.total {
                        Some(self.piece(end)?.block_id)
                    } else {
                        None
                    },
                )),
            };
            let clipped_start = previous.as_deref() == Some(id);
            let clipped_end = next.as_deref() == Some(id);
            let visible: Vec<_> = if clipped_start || clipped_end {
                original
                    .iter()
                    .enumerate()
                    .filter_map(|(i, run)| {
                        run.sources
                            .iter()
                            .any(|r| {
                                pieces
                                    .iter()
                                    .any(|p| p.sources.iter().any(|s| overlaps(r, s)))
                            })
                            .then_some(i)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let first = if clipped_start {
                visible.first().copied()
            } else {
                (!original.is_empty()).then_some(0)
            };
            let last = if clipped_end {
                visible.last().copied()
            } else {
                original.len().checked_sub(1)
            };
            if let (Some(first), Some(last)) = (first, last) {
                for i in first..=last {
                    let (mut run, length): (crate::Run, u64) = self
                        .records
                        .get_sized(
                            "runs",
                            &format!("{id}:{i}"),
                            MAX_CONTEXT_BYTES.saturating_sub(context_bytes),
                        )?
                        .ok_or_else(|| invalid("missing run record"))?;
                    context_bytes += length;
                    if (clipped_start && i == first) || (clipped_end && i == last) {
                        let mut left = usize::MAX;
                        let mut right = 0;
                        let mut position = 0;
                        for source in &run.sources {
                            for wanted in pieces.iter().flat_map(|p| &p.sources) {
                                if overlaps(source, wanted) {
                                    left = left.min(
                                        position + wanted.offset.saturating_sub(source.offset),
                                    );
                                    right = right.max(
                                        position
                                            + (wanted
                                                .offset
                                                .saturating_add(wanted.length)
                                                .min(source.offset.saturating_add(source.length))
                                                - source.offset),
                                    );
                                }
                            }
                            position += source.length;
                        }
                        let left = if clipped_start && i == first {
                            left.min(position)
                        } else {
                            0
                        };
                        let right = if clipped_end && i == last {
                            right
                        } else {
                            run.text.chars().count()
                        };
                        run = slice_run(&run, left, right);
                    }
                    block.runs.push(run);
                }
            }
            let document = pieces
                .first()
                .and_then(|p| p.sources.first())
                .map_or("context", |s| s.document.as_str())
                .to_owned();
            documents.entry(document).or_default().push(block);
        }
        book.documents = documents
            .into_iter()
            .map(|(id, blocks)| Document { id, blocks })
            .collect();
        // Reorder context blocks by their first visible piece, not hashed storage names.
        for document in &mut book.documents {
            document
                .blocks
                .sort_by_key(|b| block_order.get(b.id.as_str()).copied());
        }
        book.documents.sort_by_key(|d| {
            d.blocks
                .first()
                .and_then(|b| block_order.get(b.id.as_str()).copied())
        });
        book.chapters.clear();
        for (i, chapter) in self.metadata.chapters.iter().enumerate() {
            let chapter_start = self.chapter_starts[i];
            let chapter_end = self
                .chapter_starts
                .get(i + 1)
                .copied()
                .unwrap_or(self.total);
            if chapter_start < end && chapter_end > start {
                let mut chapter = chapter.clone();
                if chapter_start < start
                    && let Some(first) = book.pieces.first()
                {
                    chapter.start_piece = first.id.clone();
                }
                if chapter_end >= end {
                    chapter.end_piece_exclusive = None;
                }
                book.chapters.push(chapter);
            }
        }
        let notes: BTreeSet<_> = book
            .pieces
            .iter()
            .filter_map(|p| p.note_id.as_deref())
            .collect();
        let mut note_bytes = 0u64;
        for id in notes {
            match self
                .records
                .get_sized("notes", id, MAX_NOTE_BYTES.saturating_sub(note_bytes))
            {
                Ok(Some((note, size))) => {
                    note_bytes += size;
                    book.notes.push(note);
                }
                Ok(None) => {}
                Err(Error::Limit(_)) => book.notes.push(oversized_note(id)),
                Err(error) => return Err(error),
            }
        }
        Ok(BookWindow {
            book,
            start,
            total: self.total,
            image_dimensions,
        })
    }
    pub fn image_resources(&self, at: usize) -> Result<Vec<Resource>> {
        let mut result = Vec::new();
        let mut images = BufReader::new(File::open(self.root.join("images"))?);
        let mut before = std::collections::VecDeque::new();
        let mut after = Vec::new();
        loop {
            let mut bytes = [0u8; 8];
            match images.read_exact(&mut bytes) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            let index = usize::try_from(u64::from_le_bytes(bytes)).map_err(invalid)?;
            if index < at {
                before.push_back(index);
                if before.len() > 2 {
                    before.pop_front();
                }
            } else {
                after.push(index);
                if after.len() == if after.first() == Some(&at) { 3 } else { 2 } {
                    break;
                }
            }
        }
        let mut resources = BTreeSet::new();
        for index in before.into_iter().chain(after) {
            let piece = self.piece(index)?;
            let block: Block = self.named("blocks", &piece.block_id)?;
            if let Some(id) = block.resource {
                resources.insert(id);
            }
        }
        for id in resources {
            if let Some(resource) = self.records.get("resources", &id, MAX_RECORD)? {
                result.push(resource);
            } else {
                result.push(Resource {
                    id,
                    media_type: String::new(),
                    data_url: String::new(),
                });
            }
        }
        Ok(result)
    }
}
impl Drop for DiskBook {
    fn drop(&mut self) {
        if !self.persistent {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn slice_run(run: &crate::Run, start: usize, end: usize) -> crate::Run {
    let mut result = crate::Run {
        text: run
            .text
            .chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect(),
        sources: Vec::new(),
        emphasis: run.emphasis,
        strong: run.strong,
        note_id: run.note_id.clone(),
    };
    let mut position = 0usize;
    for source in &run.sources {
        let source_end = position.saturating_add(source.length);
        let left = start.max(position);
        let right = end.min(source_end);
        if left < right {
            let mut source = source.clone();
            source.offset += left - position;
            source.length = right - left;
            result.sources.push(source);
        }
        position = source_end;
    }
    result
}

fn oversized_note(id: &str) -> Note {
    let block_id = format!("{id}:oversized");
    let text = "This note exceeds the reader’s 1 MiB note window limit.".to_owned();
    Note {
        id: id.to_owned(),
        blocks: vec![Block {
            id: block_id.clone(),
            kind: "unsupported".into(),
            runs: Vec::new(),
            level: None,
            alt: Some(text.clone()),
            resource: None,
            note_id: None,
        }],
        pieces: vec![Piece {
            id: format!("{id}:oversized:piece"),
            text,
            word_id: String::new(),
            sources: Vec::new(),
            block_id,
            paragraph_end: true,
            kind: "unsupported".into(),
            note_id: None,
        }],
    }
}

/// Keep decoded image memory bounded independently of text windows.
pub(crate) fn prepare_resource(mut resource: Resource) -> Result<Resource> {
    let Some((_, encoded)) = resource.data_url.split_once(",") else {
        return Err(invalid("invalid image data URL"));
    };
    let bytes = crate::image_bytes::decode(encoded).map_err(invalid)?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    let (width, height) = reader.into_dimensions().map_err(invalid)?;
    if u64::from(width) * u64::from(height) > 40_000_000 {
        return Err(Error::Limit("image pixels"));
    }
    if width <= 1600 && height <= 1600 && bytes.len() <= 16 * 1024 * 1024 {
        return Ok(resource);
    }
    let decoded = image::load_from_memory(&bytes).map_err(invalid)?;
    let scaled = decoded.thumbnail(1600, 1600);
    let mut output = std::io::Cursor::new(Vec::new());
    scaled
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(invalid)?;
    resource.media_type = "image/png".into();
    resource.data_url = format!(
        "data:image/png;base64,{}",
        crate::image_bytes::encode(output.get_ref())
    );
    Ok(resource)
}

#[derive(Deserialize)]
struct InputPiece {
    id: String,
    text: String,
    #[serde(default)]
    word_id: Option<String>,
    #[serde(default)]
    sources: Vec<SourceSpan>,
    block_id: String,
    #[serde(default)]
    paragraph_end: bool,
    #[serde(default, deserialize_with = "present_kind")]
    kind: Option<String>,
    note_id: Option<String>,
}

fn present_kind<'de, D: de::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(decoder).map(Some)
}

struct Sink {
    records: records::Writer,
    block_kinds: BTreeMap<String, String>,
    starts: BTreeMap<String, Option<usize>>,
    late_chapters: bool,
    pieces: BufWriter<File>,
    index: BufWriter<File>,
    images: BufWriter<File>,
    offset: u64,
    total: usize,
}
impl Sink {
    fn piece(&mut self, input: InputPiece) -> Result<()> {
        let kind = input.kind.unwrap_or_else(|| {
            match self.block_kinds.get(&input.block_id).map(String::as_str) {
                Some("paragraph" | "heading" | "list_item") => "word".into(),
                Some(other) => other.into(),
                None => "unsupported".into(),
            }
        });
        let mut pieces = vec![Piece {
            id: input.id,
            text: input.text,
            word_id: input.word_id.unwrap_or_default(),
            sources: input.sources,
            block_id: input.block_id,
            paragraph_end: input.paragraph_end,
            kind,
            note_id: input.note_id,
        }];
        crate::compound::split_pieces(&mut pieces)?;
        for piece in pieces {
            if let Some(start) = self.starts.get_mut(&piece.id)
                && start.replace(self.total).is_some()
            {
                return Err(invalid("duplicate chapter piece"));
            }
            let bytes = serde_json::to_vec(&piece).map_err(invalid)?;
            if bytes.len() > MAX_PIECE_BYTES {
                return Err(Error::Limit("individual playback piece bytes"));
            }
            self.index.write_all(&self.offset.to_le_bytes())?;
            self.index.write_all(&(bytes.len() as u64).to_le_bytes())?;
            self.pieces.write_all(&bytes)?;
            self.pieces.write_all(b"\n")?;
            if piece.kind == "image" {
                self.images.write_all(&(self.total as u64).to_le_bytes())?;
            }
            self.offset += bytes.len() as u64 + 1;
            self.total += 1;
        }
        Ok(())
    }
    fn block(&mut self, mut block: Block) -> Result<()> {
        self.block_kinds
            .insert(block.id.clone(), block.kind.clone());
        let mut chunks = Vec::new();
        for run in std::mem::take(&mut block.runs) {
            if run.text.is_empty() {
                self.records
                    .insert("runs", &format!("{}:{}", block.id, chunks.len()), &run)?;
                chunks.push(run);
                continue;
            }
            let mut chars = run.text.chars();
            let mut start = 0usize;
            loop {
                let text: String = chars.by_ref().take(2048).collect();
                if text.is_empty() {
                    break;
                }
                let end = start + text.chars().count();
                let mut chunk = crate::Run {
                    text,
                    sources: Vec::new(),
                    emphasis: run.emphasis,
                    strong: run.strong,
                    note_id: run.note_id.clone(),
                };
                let mut position = 0usize;
                for source in &run.sources {
                    let source_end = position + source.length;
                    let left = start.max(position);
                    let right = end.min(source_end);
                    if left < right {
                        let mut source = source.clone();
                        source.offset += left - position;
                        source.length = right - left;
                        chunk.sources.push(source);
                    }
                    position = source_end;
                }
                self.records
                    .insert("runs", &format!("{}:{}", block.id, chunks.len()), &chunk)?;
                chunk.text.clear();
                chunks.push(chunk);
                start = end;
            }
        }
        block.runs = chunks;
        self.records.insert("blocks", &block.id, &block)
    }
}
fn overlaps(a: &SourceSpan, b: &SourceSpan) -> bool {
    a.document == b.document
        && a.node == b.node
        && a.offset < b.offset.saturating_add(b.length)
        && b.offset < a.offset.saturating_add(a.length)
}
fn empty_book() -> Book {
    Book {
        schema_version: 1,
        book_id: String::new(),
        revision: String::new(),
        title: String::new(),
        publisher_identifiers: Vec::new(),
        documents: Vec::new(),
        pieces: Vec::new(),
        chapters: Vec::new(),
        notes: Vec::new(),
        resources: Vec::new(),
        image_evidence: Vec::new(),
        diagnostics: Vec::new(),
    }
}
struct ArraySeed<T, F>(F, std::marker::PhantomData<T>);
impl<'de, T: Deserialize<'de>, F: FnMut(T) -> Result<()>> DeserializeSeed<'de> for ArraySeed<T, F> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        struct ArrayVisitor<T, F>(F, std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, F: FnMut(T) -> Result<()>> Visitor<'de> for ArrayVisitor<T, F> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an array")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                mut self,
                mut a: A,
            ) -> std::result::Result<(), A::Error> {
                while let Some(v) = a.next_element::<T>()? {
                    (self.0)(v).map_err(de::Error::custom)?;
                }
                Ok(())
            }
        }
        d.deserialize_seq(ArrayVisitor(self.0, self.1))
    }
}
struct BookSeed<'a>(&'a mut Sink);
impl<'de> DeserializeSeed<'de> for BookSeed<'_> {
    type Value = Book;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<Book, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for BookSeed<'_> {
    type Value = Book;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("normalized book")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> std::result::Result<Book, A::Error> {
        let mut book = empty_book();
        while let Some(name) = a.next_key::<String>()? {
            match name.as_str() {
                "schema_version" => book.schema_version = a.next_value()?,
                "book_id" => book.book_id = a.next_value()?,
                "revision" => book.revision = a.next_value()?,
                "title" => book.title = a.next_value()?,
                "publisher_identifiers" => book.publisher_identifiers = a.next_value()?,
                "chapters" => {
                    book.chapters = a.next_value()?;
                    self.0.late_chapters = self.0.total != 0;
                    self.0.starts = book
                        .chapters
                        .iter()
                        .map(|c| (c.start_piece.clone(), None))
                        .collect();
                }
                "diagnostics" => book.diagnostics = a.next_value()?,
                "pieces" => a.next_value_seed(ArraySeed(
                    |p: InputPiece| self.0.piece(p),
                    std::marker::PhantomData,
                ))?,
                "documents" => a.next_value_seed(DocumentsSeed(self.0))?,
                "notes" => a.next_value_seed(ArraySeed(
                    |mut n: Note| {
                        crate::compound::split_pieces(&mut n.pieces)?;
                        self.0.records.insert("notes", &n.id, &n)
                    },
                    std::marker::PhantomData,
                ))?,
                "resources" => a.next_value_seed(ArraySeed(
                    |r: Resource| {
                        let r = prepare_resource(r)?;
                        let encoded = r
                            .data_url
                            .split_once(',')
                            .ok_or_else(|| invalid("invalid image data URL"))?
                            .1;
                        let bytes = crate::image_bytes::decode(encoded).map_err(invalid)?;
                        let (width, height) = image::ImageReader::new(std::io::Cursor::new(bytes))
                            .with_guessed_format()?
                            .into_dimensions()
                            .map_err(invalid)?;
                        self.0
                            .records
                            .insert("dimensions", &r.id, &[width, height])?;
                        self.0.records.insert("resources", &r.id, &r)
                    },
                    std::marker::PhantomData,
                ))?,
                _ => {
                    a.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        if book.book_id.is_empty() || book.revision.is_empty() {
            return Err(de::Error::custom("book identity and revision required"));
        }
        Ok(book)
    }
}
struct DocumentsSeed<'a>(&'a mut Sink);
impl<'de> DeserializeSeed<'de> for DocumentsSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for DocumentsSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("documents")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> std::result::Result<(), A::Error> {
        while a.next_element_seed(DocumentSeed(self.0))?.is_some() {}
        Ok(())
    }
}
struct DocumentSeed<'a>(&'a mut Sink);
impl<'de> DeserializeSeed<'de> for DocumentSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for DocumentSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("document")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> std::result::Result<(), A::Error> {
        while let Some(key) = a.next_key::<String>()? {
            if key == "blocks" {
                a.next_value_seed(ArraySeed(
                    |b: Block| self.0.block(b),
                    std::marker::PhantomData,
                ))?;
            } else {
                a.next_value::<de::IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn chapter_field_order_and_legacy_piece_fields_are_preserved()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("verse-order-{}.json", uuid::Uuid::new_v4()));
        let book = crate::import_epub(
            Path::new("fixtures/complex.epub"),
            "order",
            &crate::ImportOptions::default(),
        )?;
        let mut value = serde_json::to_value(&book)?;
        value["pieces"][0]["word_id"] = Value::Null;
        value["pieces"][0]
            .as_object_mut()
            .ok_or("piece object")?
            .remove("kind");
        write_json(&path, &value)?;
        let early = DiskBook::open(&path)?;
        let chapters = value
            .as_object_mut()
            .ok_or("book object")?
            .remove("chapters")
            .ok_or("chapters")?;
        let mut late_json = serde_json::to_string(&value)?;
        late_json.pop();
        late_json.push_str(&format!(",\"chapters\":{chapters}}}"));
        fs::write(&path, late_json)?;
        let late = DiskBook::open(&path)?;
        assert_eq!(early.chapter_starts(), late.chapter_starts());
        assert_eq!(
            serde_json::to_value(early.load_window(0)?)?,
            serde_json::to_value(late.load_window(0)?)?
        );
        value["pieces"][0]["kind"] = Value::Null;
        write_json(&path, &value)?;
        assert!(DiskBook::open(&path).is_err());
        value["pieces"][0]["kind"] = json!("word");
        value["chapters"] = chapters;
        let duplicate = value["pieces"][0].clone();
        value["pieces"]
            .as_array_mut()
            .ok_or("pieces")?
            .push(duplicate);
        value["chapters"] = json!([{"id":"duplicate", "title":"Duplicate", "start_piece": value["pieces"][0]["id"]}]);
        write_json(&path, &value)?;
        assert!(DiskBook::open(&path).is_err());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn indexed_reads_reject_corrupt_and_truncated_records()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("disk-corruption-{}.json", uuid::Uuid::new_v4()));
        let book = crate::import_epub(
            Path::new("fixtures/complex.epub"),
            "corruption",
            &crate::ImportOptions::default(),
        )?;
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        fs::remove_file(path)?;
        // Populate the reader before changing its backing files.
        disk.load_window(0)?;
        let index_path = disk.root.join("index");
        let original = fs::read(&index_path)?;
        let mut corrupt = original.clone();
        corrupt[8..16].copy_from_slice(&(MAX_PIECE_BYTES as u64 + 1).to_le_bytes());
        fs::write(&index_path, &corrupt)?;
        assert!(matches!(
            disk.piece(0),
            Err(Error::Limit("disk piece record"))
        ));
        assert!(matches!(
            disk.load_window(0),
            Err(Error::Limit("disk piece record"))
        ));
        fs::write(&index_path, &original)?;
        File::options()
            .write(true)
            .open(disk.root.join("pieces"))?
            .set_len(0)?;
        assert!(
            matches!(disk.piece(0), Err(Error::Invalid(message)) if message == "truncated disk book")
        );
        assert!(
            matches!(disk.load_window(0), Err(Error::Invalid(message)) if message == "truncated disk book")
        );
        fs::write(index_path, [])?;
        assert!(disk.piece(0).is_err());
        assert!(disk.load_window(0).is_err());
        Ok(())
    }

    #[test]
    fn source_less_events_and_image_chapter_checkpoints_are_durable()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("disk-book-event-state-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let path = root.join("book.json");
        let state = root.join("state.json");
        let mut book = empty_book();
        book.book_id = "events".into();
        book.revision = "one".into();
        for (index, kind) in ["image", "word", "unsupported"].into_iter().enumerate() {
            book.pieces.push(Piece {
                id: format!("p{index}"),
                text: if kind == "word" {
                    "word".into()
                } else {
                    String::new()
                },
                word_id: format!("w{index}"),
                sources: if kind == "word" {
                    vec![SourceSpan {
                        document: "d".into(),
                        node: "n".into(),
                        offset: 0,
                        length: 4,
                    }]
                } else {
                    Vec::new()
                },
                block_id: format!("b{index}"),
                paragraph_end: false,
                kind: kind.into(),
                note_id: None,
            });
        }
        book.chapters.push(crate::Chapter {
            id: "illustrated-chapter".into(),
            title: "Illustrated chapter".into(),
            start_piece: "p0".into(),
            end_piece_exclusive: None,
        });
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        let chapter = disk.capture(0, &serde_json::json!({"speed":450}))?;
        for at in [0, 1, 2] {
            let captured = disk.capture(at, &chapter)?;
            assert_eq!(disk.restore(Some(&captured))?.kind, "exact");
            assert_eq!(disk.restore(Some(&captured))?.at, at);
            crate::persistence::save_record_for_book(&state, &disk, &captured, &chapter)?;
            let loaded = crate::persistence::load_record_for_book(&state, &disk)?
                .ok_or("checkpoint missing")?;
            assert_eq!(disk.restore(Some(&loaded))?.at, at);
            if at == 1 {
                assert!(captured.get("event_anchor").is_none());
            }
        }
        let mut damaged = disk.capture(1, &serde_json::json!({}))?;
        damaged["location"]["sources"] = serde_json::json!([]);
        damaged["event_anchor"] = chapter["event_anchor"].clone();
        assert_ne!(disk.restore(Some(&damaged))?.kind, "exact");
        assert!(
            crate::persistence::save_record_for_book(&state, &disk, &damaged, &chapter).is_err()
        );
        let mut wrong_event = chapter.clone();
        wrong_event["event_anchor"]["block_id"] = serde_json::json!("different-block");
        assert_ne!(disk.restore(Some(&wrong_event))?.kind, "exact");
        wrong_event = chapter;
        wrong_event["revision"] = serde_json::json!("old");
        wrong_event["location"]["revision"] = serde_json::json!("old");
        assert_ne!(disk.restore(Some(&wrong_event))?.kind, "exact");
        fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn absent_image_is_distinct_from_an_uncached_distant_image()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "disk-book-missing-image-{}.json",
            uuid::Uuid::new_v4()
        ));
        let mut book = empty_book();
        book.book_id = "missing-image".into();
        book.revision = "one".into();
        book.documents.push(Document {
            id: "d".into(),
            blocks: vec![Block {
                id: "b".into(),
                kind: "image".into(),
                runs: Vec::new(),
                level: None,
                alt: Some("Missing illustration".into()),
                resource: Some("lost.png".into()),
                note_id: None,
            }],
        });
        book.pieces.push(Piece {
            id: "p".into(),
            text: "".into(),
            word_id: "".into(),
            sources: Vec::new(),
            block_id: "b".into(),
            paragraph_end: false,
            kind: "image".into(),
            note_id: None,
        });
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        fs::remove_file(path)?;
        let resources = disk.image_resources(0)?;
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].id, "lost.png");
        assert!(resources[0].data_url.is_empty());
        Ok(())
    }
    #[test]
    fn abandoned_spools_are_reaped_without_touching_live_readers()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let base = std::env::temp_dir().join(format!("spool-lease-test-{}", uuid::Uuid::new_v4()));
        let (first, lease1) = create_spool(&base)?;
        let (second, lease2) = create_spool(&base)?;
        assert!(first.exists());
        assert!(second.exists());
        drop(lease1);
        let (third, lease3) = create_spool(&base)?;
        assert!(!first.exists());
        assert!(second.exists());
        assert!(third.exists());
        drop(lease2);
        drop(lease3);
        fs::remove_dir_all(base)?;
        Ok(())
    }
    #[test]
    fn partial_paragraph_exposes_only_selectable_pieces()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("disk-book-slice-{}.json", uuid::Uuid::new_v4()));
        let mut book = empty_book();
        book.book_id = "slice".into();
        book.revision = "one".into();
        for i in 0..1400 {
            book.pieces.push(Piece {
                id: format!("p{i}"),
                text: "word".into(),
                word_id: format!("w{i}"),
                sources: vec![SourceSpan {
                    document: "d".into(),
                    node: "n".into(),
                    offset: i * 5,
                    length: 4,
                }],
                block_id: "b".into(),
                paragraph_end: false,
                kind: "word".into(),
                note_id: None,
            });
        }
        let text = vec!["word"; 1400].join(" ");
        book.documents.push(Document {
            id: "d".into(),
            blocks: vec![Block {
                id: "b".into(),
                kind: "paragraph".into(),
                runs: vec![crate::Run {
                    text: text.clone(),
                    sources: vec![SourceSpan {
                        document: "d".into(),
                        node: "n".into(),
                        offset: 0,
                        length: text.len(),
                    }],
                    emphasis: true,
                    strong: false,
                    note_id: None,
                }],
                level: None,
                alt: None,
                resource: None,
                note_id: None,
            }],
        });
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        fs::remove_file(path)?;
        let chapter = disk.load_chapter(700)?;
        assert_eq!(chapter.start, 0);
        assert_eq!(
            chapter.book.pieces.len(),
            1400,
            "chapter pagination must include every piece beyond the playback window"
        );
        assert_eq!(
            chapter.book.documents[0].blocks[0]
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            text
        );
        let first = disk.load_window(0)?;
        let next = disk.load_window(first.start + first.book.pieces.len())?;
        assert_eq!(
            next.start,
            first.start + first.book.pieces.len(),
            "page windows must not repeat already displayed words"
        );
        let previous = disk.load_window(next.start - 1)?;
        assert_eq!(
            previous.start, first.start,
            "backward navigation must restore the same page layout"
        );
        let window = disk.load_window(700)?;
        let runs = &window.book.documents[0].blocks[0].runs;
        let actual: String = runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            actual,
            window
                .book
                .pieces
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        for run in runs {
            assert!(run.emphasis);
            assert_eq!(
                run.text.chars().count(),
                run.sources.iter().map(|s| s.length).sum::<usize>()
            );
        }
        Ok(())
    }
    #[test]
    fn giant_note_is_explicitly_unsupported_and_giant_piece_is_rejected()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("disk-book-budget-{}.json", uuid::Uuid::new_v4()));
        let mut book = empty_book();
        book.book_id = "budget".into();
        book.revision = "one".into();
        book.pieces.push(Piece {
            id: "p".into(),
            text: "note".into(),
            word_id: "w".into(),
            sources: Vec::new(),
            block_id: "b".into(),
            paragraph_end: false,
            kind: "note".into(),
            note_id: Some("n".into()),
        });
        let mut note = oversized_note("n");
        note.pieces[0].text = "x".repeat(MAX_NOTE_BYTES as usize);
        book.notes.push(note);
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        let window = disk.load_window(0)?;
        assert_eq!(window.book.notes[0].pieces[0].kind, "unsupported");
        assert!(window.book.notes[0].pieces[0].text.contains("1 MiB"));
        book.pieces[0].text = "x".repeat(MAX_PIECE_BYTES);
        write_json(&path, &book)?;
        let error = DiskBook::open(&path)
            .err()
            .ok_or("oversized piece accepted")?;
        assert!(
            error
                .to_string()
                .contains("individual playback piece bytes")
        );
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn context_preserves_original_whitespace_styles_and_source_spans()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("disk-book-style-{}.json", uuid::Uuid::new_v4()));
        let book = crate::import_epub(
            Path::new("fixtures/complex.epub"),
            "style-test",
            &crate::ImportOptions::default(),
        )?;
        assert!(book.pieces.len() < WINDOW_PIECES);
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        fs::remove_file(path)?;
        let window = disk.load_window(0)?;
        for block in window.book.documents.iter().flat_map(|d| &d.blocks) {
            let original = book
                .documents
                .iter()
                .flat_map(|d| &d.blocks)
                .find(|b| b.id == block.id)
                .ok_or("missing block")?;
            assert_eq!(
                serde_json::to_value(&block.runs)?,
                serde_json::to_value(&original.runs)?
            );
        }
        assert!(window.book.resources.is_empty());
        assert!(disk.image_resources(0)?.len() <= 5);
        Ok(())
    }
    #[test]
    fn oversized_images_are_prepared_to_a_bounded_display_size()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use base64::Engine;
        let raster = image::DynamicImage::new_rgb8(3200, 2400);
        let mut bytes = std::io::Cursor::new(Vec::new());
        raster.write_to(&mut bytes, image::ImageFormat::Png)?;
        let resource = prepare_resource(Resource {
            id: "large".into(),
            media_type: "image/png".into(),
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
            ),
        })?;
        let (_, data) = resource.data_url.split_once(',').ok_or("missing payload")?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
        let image = image::load_from_memory(&bytes)?;
        assert_eq!((image.width(), image.height()), (1600, 1200));
        Ok(())
    }
    #[test]
    fn distant_seek_capture_and_chapter_fallback_are_global()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("disk-book-test-{}.json", uuid::Uuid::new_v4()));
        let mut book = empty_book();
        book.book_id = "test".into();
        book.revision = "v1".into();
        for i in 0..1500 {
            book.pieces.push(Piece {
                id: format!("p{i}"),
                text: format!("word{i}"),
                word_id: format!("w{i}"),
                sources: vec![SourceSpan {
                    document: "d".into(),
                    node: "n".into(),
                    offset: i * 10,
                    length: 7,
                }],
                block_id: "b".into(),
                paragraph_end: false,
                kind: "word".into(),
                note_id: None,
            });
        }
        for i in [0, 500, 1000] {
            book.chapters.push(crate::Chapter {
                id: format!("c{i}"),
                title: format!("Chapter {i}"),
                start_piece: format!("p{i}"),
                end_piece_exclusive: None,
            });
        }
        write_json(&path, &book)?;
        let disk = DiskBook::open(&path)?;
        fs::remove_file(path)?;
        assert!(disk.metadata().pieces.is_empty());
        assert_eq!(disk.total_pieces(), 1500);
        let window = disk.load_window(1100)?;
        assert_eq!(window.start, 1000);
        assert_eq!(window.book.pieces.len(), 500);
        assert_eq!(window.book.chapters[0].id, "c1000");
        assert_eq!(window.book.chapters[0].start_piece, "p1000");
        let saved = disk.capture(1100, &json!({"speed":400}))?;
        assert_eq!(disk.restore(Some(&saved))?.at, 1100);
        let mut changed = saved;
        changed["revision"] = json!("old");
        assert_eq!(disk.restore(Some(&changed))?.at, 1000);
        assert_eq!(disk.piece(1499)?.id, "p1499");
        assert!(disk.piece(1500).is_err());
        std::thread::scope(|scope| -> Result<()> {
            let mut readers = Vec::new();
            for worker in 0..4 {
                let disk = &disk;
                let expected = &book.pieces;
                readers.push(scope.spawn(move || -> Result<()> {
                    for iteration in 0..32 {
                        let at = (iteration * 7919 + worker * 317) % expected.len();
                        let piece = disk.piece(at)?;
                        assert_eq!(
                            serde_json::to_value(piece).map_err(invalid)?,
                            serde_json::to_value(&expected[at]).map_err(invalid)?
                        );
                        let window = disk.load_window(at)?;
                        let end = window.start + window.book.pieces.len();
                        assert_eq!(
                            serde_json::to_value(&window.book.pieces).map_err(invalid)?,
                            serde_json::to_value(&expected[window.start..end]).map_err(invalid)?
                        );
                    }
                    Ok(())
                }));
            }
            for reader in readers {
                reader
                    .join()
                    .map_err(|_| invalid("reader thread panicked"))??;
            }
            Ok(())
        })?;
        Ok(())
    }
}
