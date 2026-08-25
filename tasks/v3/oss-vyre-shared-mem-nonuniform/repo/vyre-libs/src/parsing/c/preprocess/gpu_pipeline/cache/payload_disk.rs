use super::super::DirectivePayload;
use super::disk_common::{
    disk_cache_tmp_path, parsed_ast_cache_dir, publish_disk_cache_file,
    read_disk_cache_file_bounded, remove_disk_cache_file,
};
use super::payload_codec::{decode_payloads, encode_payloads};
use super::payload_keys::{payloads_disk_path, PayloadsCacheKey};

pub(crate) fn load_payloads_from_disk(
    key: &PayloadsCacheKey,
) -> Result<Option<Vec<DirectivePayload>>, String> {
    let dir = parsed_ast_cache_dir();
    let path = payloads_disk_path(&dir, key);
    let Some(bytes) = read_disk_cache_file_bounded(&path, "payload")? else {
        return Ok(None);
    };
    match decode_payloads(&bytes, key) {
        Ok(payloads) => Ok(Some(payloads)),
        Err(_) => {
            remove_disk_cache_file(&path, "payload");
            Ok(None)
        }
    }
}

pub(crate) fn store_payloads_to_disk(
    key: &PayloadsCacheKey,
    payloads: &[DirectivePayload],
) -> Result<(), String> {
    let dir = parsed_ast_cache_dir();
    let path = payloads_disk_path(&dir, key);
    let encoded = encode_payloads(key, payloads)?;
    let tmp = disk_cache_tmp_path(&path, "vpl");
    std::fs::write(&tmp, &encoded).map_err(|error| {
        format!(
            "vyre C GPU preprocessor disk cache could not write payload temp entry {}: {error}. Fix: repair cache directory permissions.",
            tmp.display()
        )
    })?;
    publish_disk_cache_file(&tmp, &path, "payload");
    Ok(())
}
