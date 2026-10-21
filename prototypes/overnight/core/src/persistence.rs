//! Per-publication durable state and process-lifetime exclusion shared by native readers.
use crate::location::{LocationError, SavedLocation, read_json, write_json};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// Keep this handle alive while a publication is open. Never delete lock files:
/// unlinking a locked inode would allow a second process to lock its replacement.
#[derive(Debug)]
pub struct BookLock {
    _file: fs::File,
    book_id: String,
}
impl BookLock {
    pub fn acquire(book_id: &str) -> Result<Self, String> {
        // HOME is shared by both adapters even when their scratch directories differ.
        let root = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("Cannot establish the per-user publication lock directory: HOME is unset")?
            .join(".local/state/overnight-reader/locks");
        Self::acquire_in(&root, book_id)
    }
    fn acquire_in(root: &Path, book_id: &str) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(root.join(format!("{}.lock", key(book_id))))
            .map_err(|e| format!("Cannot open publication lock: {e}"))?;
        file.try_lock()
            .map_err(|e| format!("This book is already open, or its lock is unavailable: {e}"))?;
        Ok(Self {
            _file: file,
            book_id: book_id.into(),
        })
    }
    pub fn book_id(&self) -> &str {
        &self.book_id
    }
}
fn key(id: &str) -> String {
    format!("{:x}", Sha256::digest(id.as_bytes()))
}

pub fn record_path(legacy_path: &Path, book_id: &str) -> PathBuf {
    legacy_path
        .with_extension("books")
        .join(format!("{}.json", key(book_id)))
}
fn valid_record(record: &Value, book_id: &str) -> bool {
    record.is_object()
        && record["revision"].as_str().is_some_and(|s| !s.is_empty())
        && match record.get("location") {
            Some(location) => {
                serde_json::from_value::<SavedLocation>(location.clone()).is_ok_and(|location| {
                    location.book_id == book_id
                        && location.revision == record["revision"].as_str().unwrap_or_default()
                })
            }
            None => record.get("sources").is_some_and(|sources| {
                serde_json::from_value::<Vec<crate::SourceSpan>>(sources.clone()).is_ok()
            }),
        }
}
fn candidate(path: &Path, book_id: &str) -> Result<Option<Value>, LocationError> {
    match read_json::<Value>(path) {
        Ok(record) if valid_record(&record, book_id) => Ok(Some(record)),
        Ok(_) | Err(LocationError::Json(_) | LocationError::InvalidLocation(_)) => Ok(None),
        Err(LocationError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
/// Malformed files remain untouched for diagnosis. Older valid state is preferred to
/// a chapter checkpoint; legacy whole-library state is consulted only before migration.
pub fn load_record(legacy_path: &Path, book_id: &str) -> Result<Option<Value>, LocationError> {
    let path = record_path(legacy_path, book_id);
    for candidate_path in [
        &path,
        &path.with_extension("backup.json"),
        &path.with_extension("chapter.json"),
    ] {
        if let Some(record) = candidate(candidate_path, book_id)? {
            return Ok(Some(record));
        }
    }
    if path.parent().is_some_and(|p| p.exists())
        && [
            path.clone(),
            path.with_extension("backup.json"),
            path.with_extension("chapter.json"),
        ]
        .iter()
        .any(|p| p.exists())
    {
        return Ok(None);
    }
    match read_json::<Value>(legacy_path) {
        Ok(legacy) => Ok(legacy["books"]
            .get(book_id)
            .filter(|v| valid_record(v, book_id))
            .cloned()),
        Err(LocationError::Json(_) | LocationError::InvalidLocation(_)) => Ok(None),
        Err(LocationError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
/// The caller holds the publication lock. Separate records avoid lost updates when
/// different publications are open concurrently. Legacy state is never overwritten.
pub fn save_record(
    legacy_path: &Path,
    book_id: &str,
    record: &Value,
    chapter: &Value,
) -> Result<(), LocationError> {
    if !valid_record(record, book_id) || !valid_record(chapter, book_id) {
        return Err(LocationError::InvalidLocation(
            "invalid publication checkpoint".into(),
        ));
    }
    let path = record_path(legacy_path, book_id);
    if let Some(previous) = candidate(&path, book_id)? {
        write_json(&path.with_extension("backup.json"), &previous)?;
    } else if path.exists() {
        // Keep the first damaged record without growing an unbounded evidence archive.
        let damaged = path.with_extension("damaged.json");
        if !damaged.exists() {
            fs::copy(&path, &damaged)?;
            fs::File::open(&damaged)?.sync_all()?;
        }
    }
    write_json(&path.with_extension("chapter.json"), chapter)?;
    write_json(&path, record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::process::{Command, Stdio};
    use uuid::Uuid;

    #[test]
    fn truncated_records_recover_backup_then_chapter() -> Result<(), Box<dyn std::error::Error>> {
        let root = env::temp_dir().join(format!("reader-state-{}", Uuid::new_v4()));
        let legacy = root.join("state.json");
        let record = |at| json!({"revision":"r1","sources":[],"at":at});
        save_record(&legacy, "book", &record(7), &record(0))?;
        save_record(&legacy, "book", &record(9), &record(0))?;
        assert_eq!(load_record(&legacy, "book")?, Some(record(9)));
        let path = record_path(&legacy, "book");
        fs::write(&path, b"{\"revision\":")?;
        assert_eq!(load_record(&legacy, "book")?, Some(record(7)));
        fs::write(path.with_extension("backup.json"), b"null")?;
        assert_eq!(load_record(&legacy, "book")?, Some(record(0)));
        save_record(&legacy, "book", &record(10), &record(0))?;
        assert_eq!(
            fs::read(path.with_extension("damaged.json"))?,
            b"{\"revision\":"
        );
        assert_eq!(load_record(&legacy, "book")?, Some(record(10)));
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn corrupt_startup_and_independent_records() -> Result<(), Box<dyn std::error::Error>> {
        let root = env::temp_dir().join(format!("reader-state-{}", Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let legacy = root.join("state.json");
        fs::write(&legacy, b"{")?;
        assert_eq!(load_record(&legacy, "a")?, None);
        let a = json!({"revision":"a","sources":[]});
        let b = json!({"revision":"b","sources":[]});
        save_record(&legacy, "a", &a, &a)?;
        save_record(&legacy, "b", &b, &b)?;
        assert_eq!(load_record(&legacy, "a")?, Some(a));
        assert_eq!(load_record(&legacy, "b")?, Some(b));
        assert_eq!(fs::read(&legacy)?, b"{");
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn legacy_migration_preserves_other_books() -> Result<(), Box<dyn std::error::Error>> {
        let root = env::temp_dir().join(format!("reader-state-{}", Uuid::new_v4()));
        let legacy = root.join("state.json");
        let old = json!({"revision":"old","sources":[]});
        let new = json!({"revision":"new","sources":[]});
        write_json(&legacy, &json!({"books":{"a":old,"b":old}}))?;
        let before = fs::read(&legacy)?;
        assert_eq!(load_record(&legacy, "a")?, Some(old.clone()));
        save_record(&legacy, "a", &new, &new)?;
        assert_eq!(load_record(&legacy, "a")?, Some(new));
        assert_eq!(load_record(&legacy, "b")?, Some(old));
        assert_eq!(fs::read(&legacy)?, before);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn lock_child() -> Result<(), Box<dyn std::error::Error>> {
        let Some(root) = env::var_os("READER_LOCK_TEST_ROOT") else {
            return Ok(());
        };
        let lock = BookLock::acquire_in(Path::new(&root), "same-book")?;
        fs::write(Path::new(&root).join("ready"), b"ready")?;
        std::thread::sleep(std::time::Duration::from_secs(30));
        drop(lock);
        Ok(())
    }

    #[test]
    fn process_death_releases_lock_and_other_books_can_open()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = env::temp_dir().join(format!("reader-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let mut child = Command::new(env::current_exe()?)
            .args(["--exact", "persistence::tests::lock_child"])
            .env("READER_LOCK_TEST_ROOT", &root)
            .stdout(Stdio::null())
            .spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !root.join("ready").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = root.join("ready").exists();
        let excluded = BookLock::acquire_in(&root, "same-book").is_err();
        let other = BookLock::acquire_in(&root, "different-book");
        child.kill()?;
        child.wait()?;
        assert!(ready && excluded);
        assert!(other.is_ok());
        let reopened = BookLock::acquire_in(&root, "same-book")?;
        drop(reopened);
        drop(other);
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
