use super::super::ClassifiedTokens;
use super::classified_codec::{decode_classified, encode_classified};
use super::classified_memory::ClassifiedCacheKey;
use super::disk_common::{
    classified_disk_path, disk_cache_tmp_path, parsed_ast_cache_dir, publish_disk_cache_file,
    read_disk_cache_file_bounded, remove_disk_cache_file,
};

pub(crate) fn load_classified_from_disk(
    key: &ClassifiedCacheKey,
) -> Result<Option<ClassifiedTokens>, String> {
    let dir = parsed_ast_cache_dir();
    let path = classified_disk_path(&dir, key);
    let Some(bytes) = read_disk_cache_file_bounded(&path, "classified")? else {
        return Ok(None);
    };
    match decode_classified(&bytes, key) {
        Ok(classified) => Ok(Some(classified)),
        Err(_) => {
            // Stale or collided entry: remove it so the next insert can
            // replace it without retrying decode.
            remove_disk_cache_file(&path, "classified");
            Ok(None)
        }
    }
}

pub(crate) fn store_classified_to_disk(
    key: &ClassifiedCacheKey,
    classified: &ClassifiedTokens,
) -> Result<(), String> {
    let dir = parsed_ast_cache_dir();
    let path = classified_disk_path(&dir, key);
    let encoded = encode_classified(key, classified)?;
    // Atomic publish via tempfile + rename so a concurrent reader
    // never sees a half-written entry.
    let tmp = disk_cache_tmp_path(&path, "vct");
    std::fs::write(&tmp, &encoded).map_err(|error| {
        format!(
            "vyre C GPU preprocessor disk cache could not write classified temp entry {}: {error}. Fix: repair cache directory permissions.",
            tmp.display()
        )
    })?;
    publish_disk_cache_file(&tmp, &path, "classified");
    Ok(())
}
