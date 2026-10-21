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

pub const WINDOW_PIECES: usize = 256;
const MAX_RECORD: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct BookWindow {
    pub book: Book,
    pub start: usize,
    pub total: usize,
}

#[derive(Debug)]
pub struct DiskBook {
    root: PathBuf,
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

impl DiskBook {
    /// Streams an export into a private disk spool. Spools are removed on drop.
    /// No book-sized piece index or decoded image collection is retained in memory.
    pub fn open(path: &Path) -> Result<Self> {
        let root = std::env::temp_dir().join(format!("quiet-reader-book-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        let result = Self::build(path, root.clone());
        if result.is_err() {
            let _ = fs::remove_dir_all(root);
        }
        result
    }
    fn build(path: &Path, root: PathBuf) -> Result<Self> {
        for name in ["blocks", "notes", "resources"] {
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
        sink.pieces.flush()?;
        sink.index.flush()?;
        sink.images.flush()?;
        let total = sink.total;
        drop(sink);
        let mut result = Self {
            root,
            metadata,
            total,
            chapter_starts: Vec::new(),
            buffer: Mutex::new(Vec::new()),
        };
        for chapter in &result.metadata.chapters {
            result.chapter_starts.push(
                result
                    .find_piece(&chapter.start_piece)?
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
        if length > MAX_RECORD {
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
        if saved.get("revision").and_then(Value::as_str) == Some(self.metadata.revision.as_str())
            && let Some(sources) = location.get("sources")
        {
            let sources: Vec<SourceSpan> =
                serde_json::from_value(sources.clone()).map_err(invalid)?;
            if let Some(at) = self.find_source(&sources)? {
                return Ok(session::Resume {
                    kind: "exact".into(),
                    at,
                });
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
        let index = self
            .metadata
            .chapters
            .iter()
            .position(|c| Some(c.id.as_str()) == chapter);
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
        let window = self.load_window(at)?;
        session::capture(&window.book, at - window.start, preferences).map_err(invalid)
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
        for id in ids {
            let path = self.root.join("blocks").join(key(&id));
            if !path.exists() {
                continue;
            }
            let mut block: Block = self.read(&path)?;
            let original = std::mem::take(&mut block.runs);
            let pieces: Vec<_> = book.pieces.iter().filter(|p| p.block_id == id).collect();
            for piece in &pieces {
                if piece.kind != "word" {
                    continue;
                }
                let style = original.iter().find(|r| {
                    r.sources.iter().any(|r| {
                        piece.sources.iter().any(|s| {
                            r.document == s.document
                                && r.node == s.node
                                && s.offset >= r.offset
                                && s.offset < r.offset.saturating_add(r.length)
                        })
                    })
                });
                block.runs.push(crate::Run {
                    text: format!("{} ", piece.text),
                    sources: piece.sources.clone(),
                    emphasis: style.is_some_and(|r| r.emphasis),
                    strong: style.is_some_and(|r| r.strong),
                    note_id: piece.note_id.clone(),
                });
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
        for id in notes {
            let path = self.root.join("notes").join(key(id));
            if path.exists() {
                book.notes.push(self.read(&path)?);
            }
        }
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
                if after.len() == 3 {
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
                book.resources.push(
                    serde_json::from_reader(BufReader::new(File::open(&path)?)).map_err(invalid)?,
                );
            }
        }
        Ok(BookWindow {
            book,
            start,
            total: self.total,
        })
    }
}
impl Drop for DiskBook {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
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
        for run in &mut block.runs {
            run.text.clear();
        }
        write_json(&self.root.join("blocks").join(key(&block.id)), &block)
    }
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
                    |n: Note| write_json(&self.0.root.join("notes").join(key(&n.id)), &n),
                    std::marker::PhantomData,
                ))?,
                "resources" => a.next_value_seed(ArraySeed(
                    |r: Resource| write_json(&self.0.root.join("resources").join(key(&r.id)), &r),
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
        assert_eq!(window.start, 1036);
        assert_eq!(window.book.pieces.len(), 256);
        assert_eq!(window.book.chapters[0].id, "c1000");
        assert_eq!(window.book.chapters[0].start_piece, "p1036");
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
