#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

const MAX_QUICK_CACHE_EXISTING_BYTES: u64 = 4_194_304;

pub(crate) fn write_and_commit(
    tmp: &mut fs::File,
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
) -> io::Result<()> {
    tmp.write_all(bytes)?;
    tmp.sync_all()?;
    match fs::hard_link(tmp_path, path) {
        Ok(()) => fs::remove_file(tmp_path),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let existing = read_bytes_bounded(path)?;
            fs::remove_file(tmp_path)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "cache path already exists with different content: {}. Fix: investigate hash collision or corrupt cache file.",
                        path.display()
                    ),
                ))
            }
        }
        Err(err) => Err(err),
    }
}

fn read_bytes_bounded(path: &Path) -> io::Result<Vec<u8>> {
    let mut reader = fs::File::open(path)?.take(MAX_QUICK_CACHE_EXISTING_BYTES.saturating_add(1));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_QUICK_CACHE_EXISTING_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_QUICK_CACHE_EXISTING_BYTES} byte quick-cache read cap",
                path.display()
            ),
        ));
    }
    Ok(bytes)
}
