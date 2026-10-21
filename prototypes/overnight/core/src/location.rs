//! Local publication identity and source-based reading locations.
//!
//! Store roots are caller supplied. This prototype assumes one writer per publication;
//! atomic replacement prevents partially written JSON, but does not provide locking.
use crate::{Book, SourceSpan};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_STATE_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
pub enum LocationError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidLocation(String),
}
impl fmt::Display for LocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "location storage: {e}"),
            Self::Json(e) => write!(f, "location JSON: {e}"),
            Self::InvalidLocation(e) => write!(f, "invalid location: {e}"),
        }
    }
}
impl std::error::Error for LocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::InvalidLocation(_) => None,
        }
    }
}
impl From<io::Error> for LocationError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for LocationError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// A durable source anchor and visible passage context. Treat the context as private
/// book data: persist it in local scratch storage, never in an export or report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLocation {
    pub book_id: String,
    pub revision: String,
    pub sources: Vec<SourceSpan>,
    pub text: String,
    pub chapter_id: Option<String>,
    pub chapter_title: Option<String>,
    context: Vec<String>,
    context_anchor: usize,
}

/// Changed revisions always yield a proposal, never an automatic move or state write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Recovery {
    Exact {
        piece_id: String,
    },
    ProposePassage {
        piece_id: String,
    },
    ProposeChapter {
        piece_id: String,
        chapter_title: String,
    },
    Preserve {
        reason: String,
    },
}

/// Scratch persistence. Canonical filesystem paths define the local publication:
/// replacing bytes at the same path retains identity; importing another path receives
/// another UUID even when publisher identifiers or file bytes are identical.
#[derive(Debug)]
pub struct LocalStore {
    root: PathBuf,
}
impl LocalStore {
    /// Creates a caller-selected store. No default home directory is accessed.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, LocationError> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().to_owned(),
        })
    }

    /// Returns the UUID assigned to this canonical path. Symlink aliases share an ID.
    /// The publication must exist so canonicalization has an unambiguous result.
    pub fn identity_for(&self, publication: impl AsRef<Path>) -> Result<String, LocationError> {
        let path = fs::canonicalize(publication)?;
        let key = digest(path.as_os_str().as_encoded_bytes());
        let entry = self.root.join(format!("identity-{key}.json"));
        if entry.exists() {
            return read_json(&entry);
        }
        let id = Uuid::new_v4().to_string();
        write_json(&entry, &id)?;
        Ok(id)
    }

    /// Saves a visible piece using its source spans and surrounding lexical words.
    /// Call again only after the user confirms a recovery proposal.
    pub fn save_location(
        &self,
        book: &Book,
        piece_id: &str,
    ) -> Result<SavedLocation, LocationError> {
        let location = SavedLocation::capture(book, piece_id)?;
        write_json(&self.location_path(&book.book_id), &location)?;
        Ok(location)
    }

    pub fn load_location(&self, book_id: &str) -> Result<Option<SavedLocation>, LocationError> {
        let path = self.location_path(book_id);
        match fs::metadata(&path) {
            Ok(_) => read_json(&path).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn location_path(&self, book_id: &str) -> PathBuf {
        self.root
            .join(format!("location-{}.json", digest(book_id.as_bytes())))
    }
}

impl SavedLocation {
    /// Captures a target without writing it. Piece IDs are used only to select the
    /// current piece; they are never used as cross-revision position anchors.
    pub fn capture(book: &Book, piece_id: &str) -> Result<Self, LocationError> {
        let index = book
            .pieces
            .iter()
            .position(|p| p.id == piece_id)
            .ok_or_else(|| LocationError::InvalidLocation("piece does not exist".into()))?;
        let piece = &book.pieces[index];
        let chapter = book.chapters.iter().rev().find(|chapter| {
            let start = book.pieces.iter().position(|p| p.id == chapter.start_piece);
            let end = chapter
                .end_piece_exclusive
                .as_ref()
                .and_then(|id| book.pieces.iter().position(|p| &p.id == id));
            start.is_some_and(|start| start <= index) && end.is_none_or(|end| index < end)
        });
        let words: Vec<_> = book
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == "word")
            .collect();
        let word_index = words.iter().position(|(i, _)| *i == index);
        let (context, context_anchor) = match word_index {
            Some(i) => {
                let start = i.saturating_sub(4);
                let end = (i + 8).min(words.len());
                (
                    words[start..end]
                        .iter()
                        .map(|(_, p)| p.text.clone())
                        .collect(),
                    i - start,
                )
            }
            None => (Vec::new(), 0),
        };
        Ok(Self {
            book_id: book.book_id.clone(),
            revision: book.revision.clone(),
            sources: piece.sources.clone(),
            text: piece.text.clone(),
            chapter_id: chapter.map(|c| c.id.clone()),
            chapter_title: chapter.map(|c| c.title.clone()),
            context,
            context_anchor,
        })
    }
}

/// Resolves unchanged revisions exactly. Changed-edition confidence is deliberately
/// bounded: at least eight exact surrounding words and 32 Unicode scalars, occurring
/// once in the new book. This is a conservative heuristic, not semantic matching.
/// Chapter fallback requires both the stable chapter identity and unchanged title.
/// Neither proposal mutates the saved location.
pub fn recover_location(book: &Book, saved: &SavedLocation) -> Recovery {
    let preserve = |reason: &str| Recovery::Preserve {
        reason: reason.into(),
    };
    if book.book_id != saved.book_id {
        return preserve("different local publication");
    }
    if book.revision == saved.revision {
        if saved.sources.is_empty() {
            return preserve("saved piece has no source anchor");
        }
        let mut matches = book
            .pieces
            .iter()
            .filter(|p| p.sources == saved.sources && p.text == saved.text);
        match (matches.next(), matches.next()) {
            (Some(piece), None) => {
                return Recovery::Exact {
                    piece_id: piece.id.clone(),
                };
            }
            (Some(_), Some(_)) => {
                return preserve("unchanged revision has duplicate exact anchors");
            }
            _ => {}
        }
        // Presentation may split one lexical word differently on the next import.
        // Resolve the same source start only if that lexical word still covers every
        // saved range. A missing inline text node must never become a silent move.
        let first = &saved.sources[0];
        let mut candidates = book.pieces.iter().filter(|piece| {
            piece.sources.iter().any(|source| {
                source.document == first.document
                    && source.node == first.node
                    && source.offset <= first.offset
                    && source
                        .offset
                        .checked_add(source.length)
                        .is_some_and(|end| first.offset < end)
            }) && saved
                .sources
                .iter()
                .all(|wanted| source_range_covered(book, &piece.word_id, wanted))
        });
        return match (candidates.next(), candidates.next()) {
            (Some(piece), None) => Recovery::Exact {
                piece_id: piece.id.clone(),
            },
            _ => preserve("unchanged revision has no unique exact source anchor"),
        };
    }
    let words: Vec<_> = book.pieces.iter().filter(|p| p.kind == "word").collect();
    if saved.context.len() >= 8
        && saved.context_anchor < saved.context.len()
        && saved
            .context
            .iter()
            .map(|s| s.chars().count())
            .sum::<usize>()
            >= 32
        && words.len() >= saved.context.len()
    {
        let mut matches = words.windows(saved.context.len()).filter(|window| {
            window
                .iter()
                .zip(&saved.context)
                .all(|(piece, text)| &piece.text == text)
        });
        if let (Some(window), None) = (matches.next(), matches.next()) {
            return Recovery::ProposePassage {
                piece_id: window[saved.context_anchor].id.clone(),
            };
        }
    }
    if let (Some(id), Some(title)) = (&saved.chapter_id, &saved.chapter_title) {
        let mut matches = book
            .chapters
            .iter()
            .filter(|c| &c.id == id && &c.title == title);
        if let (Some(chapter), None) = (matches.next(), matches.next())
            && book.pieces.iter().any(|p| p.id == chapter.start_piece)
        {
            return Recovery::ProposeChapter {
                piece_id: chapter.start_piece.clone(),
                chapter_title: chapter.title.clone(),
            };
        }
    }
    preserve("no unique passage or identified saved chapter; keep the old location")
}

fn source_range_covered(book: &Book, word_id: &str, wanted: &SourceSpan) -> bool {
    if word_id.is_empty() {
        return false;
    }
    let Some(wanted_end) = wanted.offset.checked_add(wanted.length) else {
        return false;
    };
    let mut spans: Vec<_> = book
        .pieces
        .iter()
        .filter(|piece| piece.word_id == word_id)
        .flat_map(|piece| &piece.sources)
        .filter(|span| span.document == wanted.document && span.node == wanted.node)
        .filter_map(|span| {
            span.offset
                .checked_add(span.length)
                .map(|end| (span.offset, end))
        })
        .collect();
    spans.sort_unstable();
    let mut covered = wanted.offset;
    for (start, end) in spans {
        if start > covered {
            break;
        }
        if end > covered {
            covered = end;
        }
        if covered >= wanted_end {
            return true;
        }
    }
    false
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Reads bounded local JSON without treating malformed state as missing.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, LocationError> {
    if fs::metadata(path)?.len() > MAX_STATE_BYTES {
        return Err(LocationError::InvalidLocation("state exceeds 1 MiB".into()));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(LocationError::InvalidLocation("state exceeds 1 MiB".into()));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
/// Atomically replaces bounded private JSON after syncing its contents.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), LocationError> {
    write_json_with_limit(path, value, MAX_STATE_BYTES)
}
/// Uses the same atomic writer for larger bounded book caches.
pub fn write_json_with_limit<T: Serialize>(
    path: &Path,
    value: &T,
    limit: u64,
) -> Result<(), LocationError> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > limit {
        return Err(LocationError::InvalidLocation(
            "JSON exceeds size limit".into(),
        ));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
    }
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    drop(file);
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(temp);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn book(text: &str) -> Result<Book, serde_json::Error> {
        let pieces: Vec<_> = text.split_whitespace().enumerate().map(|(i, text)| json!({
            "id": format!("p{i}"), "word_id": format!("w{i}"), "text": text,
            "kind":"word", "block_id":"b", "paragraph_end": false,
            "sources":[{"document":"c.xhtml","node":format!("0/{i}/text"),"offset":0,"length":text.chars().count()}]
        })).collect();
        serde_json::from_value(json!({
            "schema_version":1,"book_id":"local-a","revision":"r1","title":"Synthetic",
            "publisher_identifiers":[],"resources":[],"image_evidence":[],"diagnostics":[],
            "documents":[],"pieces":pieces,"chapters":[{"id":"c.xhtml#chapter","title":"Chapter",
            "start_piece":"p0","end_piece_exclusive":null}],"notes":[]
        }))
    }

    #[test]
    fn unchanged_restore_uses_multisource_anchor_not_piece_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut b = book("An extraordinary word")?;
        let extra = b.pieces[0].sources[0].clone();
        b.pieces[1].sources.push(extra);
        let saved = SavedLocation::capture(&b, "p1")?;
        b.pieces[1].id = "regenerated-id".into();
        assert_eq!(
            recover_location(&b, &saved),
            Recovery::Exact {
                piece_id: "regenerated-id".into()
            }
        );
        b.pieces[1].sources.pop();
        assert!(matches!(
            recover_location(&b, &saved),
            Recovery::Preserve { .. }
        ));
        Ok(())
    }

    #[test]
    fn display_splitting_preserves_source_position() -> Result<(), Box<dyn std::error::Error>> {
        let mut b = book("well-known")?;
        let whole = SavedLocation::capture(&b, "p0")?;
        let mut second = b.pieces[0].clone();
        b.pieces[0].text = "well-".into();
        b.pieces[0].sources[0].length = 5;
        second.id = "p1".into();
        second.text = "known".into();
        second.sources[0].offset = 5;
        second.sources[0].length = 5;
        b.pieces.push(second);
        assert_eq!(
            recover_location(&b, &whole),
            Recovery::Exact {
                piece_id: "p0".into()
            }
        );
        let split = SavedLocation::capture(&b, "p1")?;
        let unsplit = book("well-known")?;
        assert_eq!(
            recover_location(&unsplit, &split),
            Recovery::Exact {
                piece_id: "p0".into()
            }
        );
        Ok(())
    }

    #[test]
    fn changed_unique_passage_is_only_a_proposal() -> Result<(), Box<dyn std::error::Error>> {
        let text = "Morning light crossed the silent courtyard beside the ancient flowering trees";
        let b = book(text)?;
        let saved = SavedLocation::capture(&b, "p4")?;
        let mut changed = book(&format!("New introduction words {text}"))?;
        changed.revision = "r2".into();
        assert_eq!(
            recover_location(&changed, &saved),
            Recovery::ProposePassage {
                piece_id: "p7".into()
            }
        );
        assert_eq!(saved.revision, "r1");
        assert_eq!(saved.text, "silent");
        Ok(())
    }

    #[test]
    fn ambiguous_passage_offers_identified_chapter_or_preserves()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = "Morning light crossed the silent courtyard beside the ancient flowering trees";
        let b = book(text)?;
        let saved = SavedLocation::capture(&b, "p4")?;
        let mut changed = book(&format!("{text} {text}"))?;
        changed.revision = "r2".into();
        assert_eq!(
            recover_location(&changed, &saved),
            Recovery::ProposeChapter {
                piece_id: "p0".into(),
                chapter_title: "Chapter".into()
            }
        );
        changed.chapters[0].id = "different.xhtml#chapter".into();
        assert!(matches!(
            recover_location(&changed, &saved),
            Recovery::Preserve { .. }
        ));
        changed.book_id = "unrelated-book".into();
        assert!(matches!(
            recover_location(&changed, &saved),
            Recovery::Preserve { .. }
        ));
        Ok(())
    }

    #[test]
    fn malformed_and_oversized_state_are_errors() -> Result<(), Box<dyn std::error::Error>> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scratch")
            .join(format!("reader-location-test-{}", Uuid::new_v4()));
        let store = LocalStore::open(&dir)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let b = book("An existing passage")?;
            assert!(matches!(
                store.save_location(&b, "absent"),
                Err(LocationError::InvalidLocation(_))
            ));
            assert!(store.load_location(&b.book_id)?.is_none());
            let path = store.location_path(&b.book_id);
            fs::write(&path, b"{")?;
            assert!(matches!(
                store.load_location(&b.book_id),
                Err(LocationError::Json(_))
            ));
            fs::File::create(&path)?.set_len(MAX_STATE_BYTES + 1)?;
            assert!(matches!(
                store.load_location(&b.book_id),
                Err(LocationError::InvalidLocation(_))
            ));
            Ok(())
        })();
        fs::remove_dir_all(&dir)?;
        result
    }

    #[test]
    fn local_identity_and_recovery_survive_store_reopen() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scratch")
            .join(format!("reader-location-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let a = dir.join("a.epub");
            let b = dir.join("b.epub");
            fs::write(&a, b"same publisher id")?;
            fs::write(&b, b"same publisher id")?;
            let store = LocalStore::open(dir.join("scratch"))?;
            let id = store.identity_for(&a)?;
            assert_ne!(id, store.identity_for(&b)?);
            fs::write(&a, b"replacement edition")?;
            let reopened = LocalStore::open(dir.join("scratch"))?;
            assert_eq!(id, reopened.identity_for(&a)?);
            let mut publication = book("One visible passage")?;
            publication.book_id = id;
            reopened.save_location(&publication, "p1")?;
            let saved = reopened
                .load_location(&publication.book_id)?
                .ok_or("missing saved location")?;
            assert_eq!(
                recover_location(&publication, &saved),
                Recovery::Exact {
                    piece_id: "p1".into()
                }
            );
            Ok(())
        })();
        fs::remove_dir_all(&dir)?;
        result
    }
}
