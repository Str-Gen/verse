use super::{MAX_RECORD, invalid};
use crate::Result;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Mutex,
};

const ENTRY_BYTES: u64 = 48;
const RESIDENT_INDEX_BYTES: u64 = 1024 * 1024;

fn key(group: &str, id: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(group);
    hash.update([0]);
    hash.update(id);
    hash.finalize().into()
}

pub(super) struct Writer {
    data: BufWriter<File>,
    entries: Vec<([u8; 32], u64, u64)>,
    offset: u64,
    buffer: Vec<u8>,
}

impl Writer {
    pub(super) fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            data: BufWriter::new(File::create(root.join("records"))?),
            entries: Vec::new(),
            offset: 0,
            buffer: Vec::new(),
        })
    }

    pub(super) fn insert(&mut self, group: &str, id: &str, value: &impl Serialize) -> Result<()> {
        self.buffer.clear();
        serde_json::to_writer(&mut self.buffer, value).map_err(invalid)?;
        let length = self.buffer.len() as u64;
        if length > MAX_RECORD {
            return Err(crate::Error::Limit("disk book record"));
        }
        self.entries.push((key(group, id), self.offset, length));
        self.data.write_all(&self.buffer)?;
        self.offset = self
            .offset
            .checked_add(length)
            .ok_or_else(|| invalid("record offset overflow"))?;
        Ok(())
    }

    pub(super) fn finish(mut self, root: &Path) -> Result<()> {
        self.data.flush()?;
        // Stable sorting preserves the former last-write-wins behavior for duplicate IDs.
        self.entries.sort_by_key(|entry| entry.0);
        let mut index = BufWriter::new(File::create(root.join("record-index"))?);
        for (i, &(key, offset, length)) in self.entries.iter().enumerate() {
            if self.entries.get(i + 1).is_some_and(|next| next.0 == key) {
                continue;
            }
            index.write_all(&key)?;
            index.write_all(&offset.to_le_bytes())?;
            index.write_all(&length.to_le_bytes())?;
        }
        index.flush()?;
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct Records(Mutex<Reader>);

#[derive(Debug)]
struct Reader {
    index: Index,
    data: File,
    data_length: u64,
    count: u64,
    buffer: Vec<u8>,
}

#[derive(Debug)]
enum Index {
    Resident(Vec<[u8; ENTRY_BYTES as usize]>),
    Disk(File),
}

impl Index {
    fn entry(&mut self, at: u64) -> Result<[u8; ENTRY_BYTES as usize]> {
        match self {
            Self::Resident(entries) => entries
                .get(at as usize)
                .copied()
                .ok_or_else(|| invalid("record index outside book")),
            Self::Disk(file) => {
                file.seek(SeekFrom::Start(at * ENTRY_BYTES))?;
                let mut entry = [0; ENTRY_BYTES as usize];
                file.read_exact(&mut entry)?;
                Ok(entry)
            }
        }
    }
}

impl Records {
    pub(super) fn open(root: &Path) -> Result<Self> {
        let mut index = File::open(root.join("record-index"))?;
        let length = index.metadata()?.len();
        if !length.is_multiple_of(ENTRY_BYTES) {
            return Err(invalid("truncated record index"));
        }
        let index = if length <= RESIDENT_INDEX_BYTES {
            let mut entries = vec![[0; ENTRY_BYTES as usize]; (length / ENTRY_BYTES) as usize];
            index.read_exact(entries.as_flattened_mut())?;
            Index::Resident(entries)
        } else {
            Index::Disk(index)
        };
        let data = File::open(root.join("records"))?;
        let data_length = data.metadata()?.len();
        Ok(Self(Mutex::new(Reader {
            index,
            data,
            data_length,
            count: length / ENTRY_BYTES,
            buffer: Vec::new(),
        })))
    }

    pub(super) fn get<T: DeserializeOwned>(
        &self,
        group: &str,
        id: &str,
        limit: u64,
    ) -> Result<Option<T>> {
        self.get_sized(group, id, limit)
            .map(|record| record.map(|(value, _)| value))
    }

    pub(super) fn get_sized<T: DeserializeOwned>(
        &self,
        group: &str,
        id: &str,
        limit: u64,
    ) -> Result<Option<(T, u64)>> {
        let mut reader = self
            .0
            .lock()
            .map_err(|_| invalid("record reader poisoned"))?;
        let wanted = key(group, id);
        let mut low = 0;
        let mut high = reader.count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = reader.index.entry(middle)?;
            match entry[..32].cmp(&wanted) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    let offset = u64::from_le_bytes(entry[32..40].try_into().map_err(invalid)?);
                    let length = u64::from_le_bytes(entry[40..].try_into().map_err(invalid)?);
                    if length > limit.min(MAX_RECORD) {
                        return Err(crate::Error::Limit("disk book record"));
                    }
                    if offset
                        .checked_add(length)
                        .is_none_or(|end| end > reader.data_length)
                    {
                        return Err(invalid("truncated disk record"));
                    }
                    reader.data.seek(SeekFrom::Start(offset))?;
                    let Reader { data, buffer, .. } = &mut *reader;
                    buffer.resize(length as usize, 0);
                    data.read_exact(buffer)?;
                    return serde_json::from_slice(buffer)
                        .map(|value| Some((value, length)))
                        .map_err(invalid);
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_and_disk_indexes_read_identically_and_reject_truncation()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        for count in [32, RESIDENT_INDEX_BYTES / ENTRY_BYTES + 1] {
            let root = std::env::temp_dir().join(format!("verse-records-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root)?;
            let mut writer = Writer::new(&root)?;
            for at in 0..count {
                writer.insert("runs", &at.to_string(), &at)?;
            }
            writer.finish(&root)?;
            let records = Records::open(&root)?;
            for at in [0, count / 2, count - 1] {
                assert_eq!(
                    records.get::<u64>("runs", &at.to_string(), MAX_RECORD)?,
                    Some(at)
                );
            }
            assert_eq!(records.get::<u64>("notes", "0", MAX_RECORD)?, None);
            assert!(records.get::<u64>("runs", "0", 0).is_err());
            File::options()
                .write(true)
                .open(root.join("records"))?
                .set_len(0)?;
            assert!(records.get::<u64>("runs", "0", MAX_RECORD).is_err());
            drop(records);
            std::fs::remove_dir_all(root)?;
        }
        Ok(())
    }
}
