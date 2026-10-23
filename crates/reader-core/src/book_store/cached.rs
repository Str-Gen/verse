use super::*;
use crate::cache::fingerprint;

// Bump when the stored representation or normalization semantics change.
const FORMAT: &str = "indexed-books-v1";
const FILES: [&str; 5] = ["pieces", "index", "images", "records", "record-index"];
const RETAINED_BOOKS: usize = 8;

#[derive(Serialize, Deserialize)]
struct Manifest {
    metadata: Book,
    total: usize,
    chapter_starts: Vec<usize>,
    checksums: [String; 5],
}

impl DiskBook {
    /// Reuses a content-checked store. Inactive entries are bounded to eight books.
    pub fn open_cached(path: &Path) -> Result<Self> {
        let base = cache_directory()?.with_file_name(FORMAT);
        Self::open_cached_in(path, &base)
    }

    fn open_cached_in(path: &Path, base: &Path) -> Result<Self> {
        private_directory(base)?;
        let digest = fingerprint(path)?;
        let guard = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(base.join("cache.lock"))?;
        guard.lock()?;
        let root = base.join(&digest);
        if root.exists() {
            if let Ok(book) = Self::reuse(&root) {
                // Only the age marker changes; the data files remain immutable.
                File::create(root.join("used"))?;
                prune(base, &root)?;
                return Ok(book);
            }
            if let Ok(lease) = File::open(root.join("lease"))
                && lease.try_lock().is_err()
            {
                // A live reader owns this snapshot. Do not replace its files.
                return Self::open(path);
            }
            fs::remove_dir_all(&root)?;
        }
        private_directory(&root)?;
        let lease = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.join("lease"))?;
        lease.lock_shared()?;
        let mut book = Self::build(path, root.clone(), lease)?;
        if fingerprint(path)? != digest {
            return Err(invalid("book changed while building its cache"));
        }
        let checksums = FILES
            .map(|name| fingerprint(&root.join(name)))
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid("cache checksums"))?;
        let manifest = Manifest {
            metadata: book.metadata.clone(),
            total: book.total,
            chapter_starts: book.chapter_starts.clone(),
            checksums,
        };
        let mut file = BufWriter::new(File::create(root.join("manifest.json"))?);
        serde_json::to_writer(&mut file, &manifest).map_err(invalid)?;
        file.flush()?;
        fs::write(
            root.join("complete"),
            fingerprint(&root.join("manifest.json"))?,
        )?;
        File::create(root.join("used"))?;
        book.persistent = true;
        prune(base, &root)?;
        Ok(book)
    }

    fn reuse(root: &Path) -> Result<Self> {
        if fs::metadata(root.join("complete"))?.len() != 64
            || fs::read_to_string(root.join("complete"))?
                != fingerprint(&root.join("manifest.json"))?
        {
            return Err(invalid("cached metadata checksum mismatch"));
        }
        let file = File::open(root.join("manifest.json"))?;
        if file.metadata()?.len() > MAX_METADATA_BYTES as u64 * 2 {
            return Err(Error::Limit("cached book metadata"));
        }
        let manifest: Manifest = serde_json::from_reader(BufReader::new(file)).map_err(invalid)?;
        for (name, expected) in FILES.iter().zip(&manifest.checksums) {
            if fingerprint(&root.join(name))? != *expected {
                return Err(invalid("cached book checksum mismatch"));
            }
        }
        if manifest.chapter_starts.len() != manifest.metadata.chapters.len()
            || manifest
                .chapter_starts
                .iter()
                .any(|&at| at >= manifest.total)
            || manifest.total.checked_mul(16).map(|n| n as u64)
                != Some(fs::metadata(root.join("index"))?.len())
        {
            return Err(invalid("invalid cached book index"));
        }
        let lease = File::open(root.join("lease"))?;
        lease.lock_shared()?;
        Ok(Self {
            root: root.to_owned(),
            _lease: lease,
            persistent: true,
            metadata: manifest.metadata,
            total: manifest.total,
            chapter_starts: manifest.chapter_starts,
            records: records::Records::open(root)?,
            reader: Mutex::new(PieceReader {
                index: File::open(root.join("index"))?,
                pieces: BufReader::new(File::open(root.join("pieces"))?),
                buffer: Vec::new(),
            }),
        })
    }
}

fn prune(base: &Path, current: &Path) -> Result<()> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.len() == 64
            && name.bytes().all(|b| b.is_ascii_hexdigit())
            && entry.file_type()?.is_dir()
        {
            let age = fs::metadata(entry.path().join("used"))
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            entries.push((age, entry.path()));
        }
    }
    entries.sort_by_key(|(age, _)| *age);
    let mut excess = entries.len().saturating_sub(RETAINED_BOOKS);
    for (_, path) in entries {
        if excess == 0 {
            break;
        }
        if path == current {
            continue;
        }
        if let Ok(lease) = File::open(path.join("lease"))
            && lease.try_lock().is_err()
        {
            continue;
        }
        fs::remove_dir_all(path)?;
        excess -= 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_revision_corruption_and_live_leases()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("verse-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let path = root.join("book.json");
        let mut book = crate::import_epub(
            Path::new("fixtures/complex.epub"),
            "cache",
            &crate::ImportOptions::default(),
        )?;
        write_json(&path, &book)?;
        let base = root.join("cache");
        let first = DiskBook::open_cached_in(&path, &base)?;
        let expected = serde_json::to_value(first.load_window(0)?)?;
        let second = DiskBook::open_cached_in(&path, &base)?;
        assert_eq!(first.root, second.root);
        assert_eq!(expected, serde_json::to_value(second.load_window(0)?)?);
        let cached_root = first.root.clone();
        // Corruption in an in-use cache must not unlink the live reader's snapshot.
        fs::write(cached_root.join("records"), b"broken")?;
        let repaired = DiskBook::open_cached_in(&path, &base)?;
        assert_ne!(repaired.root, cached_root);
        assert!(cached_root.exists());
        assert_eq!(expected, serde_json::to_value(repaired.load_window(0)?)?);
        drop((first, second, repaired));
        let repaired = DiskBook::open_cached_in(&path, &base)?;
        assert_eq!(expected, serde_json::to_value(repaired.load_window(0)?)?);
        drop(repaired);
        fs::write(cached_root.join("manifest.json"), b"{}")?;
        assert_eq!(
            expected,
            serde_json::to_value(DiskBook::open_cached_in(&path, &base)?.load_window(0)?)?
        );
        // Same path, changed contents must select a new cache regardless of timestamps.
        book.revision = "revised".into();
        write_json(&path, &book)?;
        let revised = DiskBook::open_cached_in(&path, &base)?;
        assert_ne!(revised.root, cached_root);
        assert_eq!(revised.metadata().revision, "revised");
        drop(revised);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn pruning_keeps_live_readers_and_bounds_inactive_entries()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("verse-prune-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let path = root.join("book.json");
        let mut book = empty_book();
        book.book_id = "prune".into();
        let base = root.join("cache");
        let mut live = Vec::new();
        for index in 0..10 {
            book.revision = index.to_string();
            write_json(&path, &book)?;
            live.push(DiskBook::open_cached_in(&path, &base)?);
        }
        assert!(live.iter().all(|book| book.root.exists()));
        live.clear();
        drop(DiskBook::open_cached_in(&path, &base)?);
        assert_eq!(
            fs::read_dir(&base)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count(),
            RETAINED_BOOKS
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
