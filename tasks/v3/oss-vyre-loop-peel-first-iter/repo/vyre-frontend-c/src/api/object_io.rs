use std::io::Read as _;
use std::path::Path;

use crate::object_format::{parse_embedded_vyrecob2, Vyrecob2};

const MAX_FRONTEND_OBJECT_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn decode_embedded_object<T, F>(object_bytes: &[u8], decode: F) -> Result<T, String>
where
    F: for<'container> FnOnce(&Vyrecob2<'container>) -> Result<T, String>,
{
    let container = parse_embedded_vyrecob2(object_bytes)?;
    decode(&container)
}

pub(crate) fn read_object_file<T, F>(path: &Path, decode: F) -> Result<T, String>
where
    F: FnOnce(&[u8]) -> Result<T, String>,
{
    let bytes = read_object_bytes_bounded(path)?;
    decode(&bytes)
}

pub(crate) fn read_object_bytes_bounded(path: &Path) -> Result<Vec<u8>, String> {
    read_object_bytes_bounded_with_limit(path, MAX_FRONTEND_OBJECT_BYTES)
}

fn read_object_bytes_bounded_with_limit(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "vyre-frontend-c: read object metadata {}: {error}",
            path.display()
        )
    })?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "vyre-frontend-c: object {} is {} bytes; maximum accepted object input is {max_bytes} bytes. Fix: reject, shard, or regenerate this object before decoding.",
            path.display(),
            metadata.len()
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "vyre-frontend-c: object {} is {} bytes and exceeds host addressable memory. Fix: reject or shard this object before decoding.",
            path.display(),
            metadata.len()
        )
    })?;
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("vyre-frontend-c: open object {}: {error}", path.display()))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("vyre-frontend-c: read object {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "vyre-frontend-c: object {} exceeded {max_bytes} bytes while reading. Fix: reject, shard, or regenerate this object before decoding.",
            path.display()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_object_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "vyre_frontend_c_object_io_{}_{}_{}",
            name,
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        path
    }

    #[test]
    fn bounded_object_reader_accepts_file_at_limit() {
        let path = temp_object_path("at_limit");
        std::fs::write(&path, [0xA5u8; 16]).expect("Fix: temp object fixture must be writable");

        let bytes = read_object_bytes_bounded_with_limit(&path, 16)
            .expect("Fix: object at byte cap must be accepted");

        assert_eq!(bytes.len(), 16);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn bounded_object_reader_rejects_file_over_limit_before_decode() {
        let path = temp_object_path("over_limit");
        std::fs::write(&path, [0x5Au8; 17]).expect("Fix: temp object fixture must be writable");

        let error = read_object_bytes_bounded_with_limit(&path, 16)
            .expect_err("Fix: oversized object must be rejected before decode");

        assert!(error.contains("maximum accepted object input is 16 bytes"));
        std::fs::remove_file(path).ok();
    }
}
