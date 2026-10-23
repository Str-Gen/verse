//! Content checks for rebuildable local caches. Timestamps alone never establish freshness.
use crate::Result;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub(crate) fn fingerprint(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
