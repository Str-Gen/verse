//! Disk-backed normalized books. Only a bounded playback window is resident.
use crate::{Block, Book, Document, Error, Note, Piece, Resource, Result, SourceSpan, session};
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
}

#[derive(Debug)]
pub struct DiskBook {
    root: PathBuf,
    _lease: File,
    metadata: Book,
    total: usize,
    chapter_starts: Vec<usize>,
    buffer: Mutex<Vec<u8>>,
}

fn invalid(error: impl fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}
fn key(id: &str) -> String {
    format!("{:x}", Sha256::digest(id.as_bytes()))
}
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
        for name in ["blocks", "runs", "notes", "resources"] {
            fs::create_dir(root.join(name))?;
        }
        let mut sink = Sink {
            root: &root,
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
        drop(sink);
        let mut result = Self {
            root,
            _lease: lease,
            metadata,
            total,
            chapter_starts: Vec::new(),
            buffer: Mutex::new(Vec::new()),
        };
        let mut starts: BTreeMap<String, Option<usize>> = result
            .metadata
            .chapters
            .iter()
            .map(|c| (c.start_piece.clone(), None))
            .collect();
        let reader = BufReader::new(File::open(result.root.join("pieces"))?);
        for (index, piece) in serde_json::Deserializer::from_reader(reader)
            .into_iter::<Piece>()
            .enumerate()
        {
            let piece = piece.map_err(invalid)?;
            if let Some(start) = starts.get_mut(&piece.id)
                && start.replace(index).is_some()
            {
                return Err(invalid("duplicate chapter piece"));
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
    fn read<T: de::DeserializeOwned>(&self, path: &Path) -> Result<T> {
        let file = File::open(path)?;
        let size = file.metadata()?.len();
        if size > MAX_RECORD {
            return Err(Error::Limit("disk book record"));
        }
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| invalid("book buffer poisoned"))?;
        buffer.clear();
        file.take(MAX_RECORD + 1).read_to_end(&mut buffer)?;
        serde_json::from_slice(&buffer).map_err(invalid)
    }
    fn named<T: de::DeserializeOwned>(&self, group: &str, id: &str) -> Result<T> {
        self.read(&self.root.join(group).join(key(id)))
    }
    pub fn piece(&self, index: usize) -> Result<Piece> {
        if index >= self.total {
            return Err(invalid("piece index outside book"));
        }
        let mut offsets = File::open(self.root.join("index"))?;
        offsets.seek(SeekFrom::Start(
            (index as u64)
                .checked_mul(16)
                .ok_or(Error::Limit("piece index"))?,
        ))?;
        let mut bytes = [0u8; 16];
        offsets.read_exact(&mut bytes)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&bytes[..8]);
        let offset = u64::from_le_bytes(a);
        a.copy_from_slice(&bytes[8..]);
        let length = u64::from_le_bytes(a);
        if length > MAX_PIECE_BYTES as u64 {
            return Err(Error::Limit("disk piece record"));
        }
        let mut data = File::open(self.root.join("pieces"))?;
        data.seek(SeekFrom::Start(offset))?;
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| invalid("book buffer poisoned"))?;
        buffer.clear();
        data.take(length).read_to_end(&mut buffer)?;
        if buffer.len() as u64 != length {
            return Err(invalid("truncated disk book"));
        }
        serde_json::from_slice(&buffer).map_err(invalid)
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
        let start = at.saturating_sub(WINDOW_PIECES / 4);
        let end = start.saturating_add(WINDOW_PIECES).min(self.total);
        let mut book = self.metadata.clone();
        for i in start..end {
            book.pieces.push(self.piece(i)?);
        }
        let ids: BTreeSet<_> = book.pieces.iter().map(|p| p.block_id.clone()).collect();
        let mut documents: BTreeMap<String, Vec<Block>> = BTreeMap::new();
        let mut context_bytes = 0u64;
        for id in ids {
            let path = self.root.join("blocks").join(key(&id));
            if !path.exists() {
                continue;
            }
            if fs::metadata(&path)?.len() > MAX_CONTEXT_BYTES {
                return Err(Error::Limit("block styling metadata bytes"));
            }
            let mut block: Block = self.read(&path)?;
            let original = std::mem::take(&mut block.runs);
            let pieces: Vec<_> = book.pieces.iter().filter(|p| p.block_id == id).collect();
            let clipped_start = start > 0 && self.piece(start - 1)?.block_id == id;
            let clipped_end = end < self.total && self.piece(end)?.block_id == id;
            let visible: Vec<_> = original
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
                .collect();
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
                    let run_path = self.root.join("runs").join(key(&format!("{id}:{i}")));
                    context_bytes = context_bytes.saturating_add(fs::metadata(&run_path)?.len());
                    if context_bytes > MAX_CONTEXT_BYTES {
                        return Err(Error::Limit("visible context bytes"));
                    }
                    let mut run: crate::Run = self.read(&run_path)?;
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
                .sort_by_key(|b| book.pieces.iter().position(|p| p.block_id == b.id));
        }
        book.documents.sort_by_key(|d| {
            d.blocks
                .first()
                .and_then(|b| book.pieces.iter().position(|p| p.block_id == b.id))
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
            let path = self.root.join("notes").join(key(id));
            if !path.exists() {
                continue;
            }
            let size = fs::metadata(&path)?.len();
            if size > MAX_NOTE_BYTES.saturating_sub(note_bytes) {
                book.notes.push(oversized_note(id));
            } else {
                note_bytes += size;
                book.notes.push(self.read(&path)?);
            }
        }
        Ok(BookWindow {
            book,
            start,
            total: self.total,
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
            let path = self.root.join("resources").join(key(&id));
            if path.exists() {
                result.push(
                    serde_json::from_reader(BufReader::new(File::open(&path)?)).map_err(invalid)?,
                );
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
        let _ = fs::remove_dir_all(&self.root);
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
    use base64::Engine;
    let Some((_, encoded)) = resource.data_url.split_once(",") else {
        return Err(invalid("invalid image data URL"));
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(invalid)?;
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
        base64::engine::general_purpose::STANDARD.encode(output.into_inner())
    );
    Ok(resource)
}

struct Sink<'a> {
    root: &'a Path,
    pieces: BufWriter<File>,
    index: BufWriter<File>,
    images: BufWriter<File>,
    offset: u64,
    total: usize,
}
impl Sink<'_> {
    fn piece(&mut self, value: Value) -> Result<()> {
        let mut value = value;
        if value.get("kind").is_none() {
            let id = value["block_id"].as_str().unwrap_or_default();
            let path = self.root.join("blocks").join(key(id));
            let block: Option<Block> = if path.exists() {
                Some(serde_json::from_reader(BufReader::new(File::open(path)?)).map_err(invalid)?)
            } else {
                None
            };
            let kind = block
                .as_ref()
                .map(|b| b.kind.as_str())
                .unwrap_or("unsupported");
            value["kind"] = serde_json::json!(match kind {
                "paragraph" | "heading" | "list_item" => "word",
                other => other,
            });
        }
        if value.get("word_id").is_some_and(Value::is_null) {
            value["word_id"] = serde_json::json!("");
        }
        let piece: Piece = serde_json::from_value(value).map_err(invalid)?;
        let mut mini = empty_book();
        mini.pieces.push(piece);
        crate::compound::split_book(&mut mini)?;
        for piece in mini.pieces {
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
        let mut chunks = Vec::new();
        for run in std::mem::take(&mut block.runs) {
            if run.text.is_empty() {
                write_json(
                    &self
                        .root
                        .join("runs")
                        .join(key(&format!("{}:{}", block.id, chunks.len()))),
                    &run,
                )?;
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
                write_json(
                    &self
                        .root
                        .join("runs")
                        .join(key(&format!("{}:{}", block.id, chunks.len()))),
                    &chunk,
                )?;
                chunk.text.clear();
                chunks.push(chunk);
                start = end;
            }
        }
        block.runs = chunks;
        write_json(&self.root.join("blocks").join(key(&block.id)), &block)
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
struct BookSeed<'a, 'b>(&'a mut Sink<'b>);
impl<'de> DeserializeSeed<'de> for BookSeed<'_, '_> {
    type Value = Book;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<Book, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for BookSeed<'_, '_> {
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
                "chapters" => book.chapters = a.next_value()?,
                "diagnostics" => book.diagnostics = a.next_value()?,
                "pieces" => a.next_value_seed(ArraySeed(
                    |p: Value| self.0.piece(p),
                    std::marker::PhantomData,
                ))?,
                "documents" => a.next_value_seed(DocumentsSeed(self.0))?,
                "notes" => a.next_value_seed(ArraySeed(
                    |mut n: Note| {
                        crate::compound::split_pieces(&mut n.pieces)?;
                        write_json(&self.0.root.join("notes").join(key(&n.id)), &n)
                    },
                    std::marker::PhantomData,
                ))?,
                "resources" => a.next_value_seed(ArraySeed(
                    |r: Resource| {
                        let r = prepare_resource(r)?;
                        write_json(&self.0.root.join("resources").join(key(&r.id)), &r)
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
struct DocumentsSeed<'a, 'b>(&'a mut Sink<'b>);
impl<'de> DeserializeSeed<'de> for DocumentsSeed<'_, '_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for DocumentsSeed<'_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("documents")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> std::result::Result<(), A::Error> {
        while a.next_element_seed(DocumentSeed(self.0))?.is_some() {}
        Ok(())
    }
}
struct DocumentSeed<'a, 'b>(&'a mut Sink<'b>);
impl<'de> DeserializeSeed<'de> for DocumentSeed<'_, '_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for DocumentSeed<'_, '_> {
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
        assert_eq!(window.start, 972);
        assert_eq!(window.book.pieces.len(), 512);
        assert_eq!(window.book.chapters[0].id, "c500");
        assert_eq!(window.book.chapters[0].start_piece, "p972");
        let saved = disk.capture(1100, &json!({"speed":400}))?;
        assert_eq!(disk.restore(Some(&saved))?.at, 1100);
        let mut changed = saved;
        changed["revision"] = json!("old");
        assert_eq!(disk.restore(Some(&changed))?.at, 1000);
        assert_eq!(disk.piece(1499)?.id, "p1499");
        assert!(disk.piece(1500).is_err());
        Ok(())
    }
}
