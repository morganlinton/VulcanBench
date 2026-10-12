use super::super::ClassifiedTokens;
use super::classified_memory::ClassifiedCacheKey;
use super::disk_common::CLASSIFIED_DISK_MAGIC;

pub(crate) fn encode_classified(
    key: &ClassifiedCacheKey,
    classified: &ClassifiedTokens,
) -> Result<Vec<u8>, String> {
    let path_bytes = key
        .path
        .as_os_str()
        .to_string_lossy()
        .into_owned()
        .into_bytes();
    let encoded_len = classified_encoded_len(&path_bytes, classified)?;
    let mut out = Vec::new();
    reserve_encode_bytes(&mut out, encoded_len, "classified cache entry")?;
    out.extend_from_slice(CLASSIFIED_DISK_MAGIC);
    out.extend_from_slice(&encode_len_u64(path_bytes.len(), "classified path")?.to_le_bytes());
    out.extend_from_slice(&path_bytes);
    out.extend_from_slice(
        &encode_len_u64(key.source_len, "classified source length")?.to_le_bytes(),
    );
    out.extend_from_slice(&key.source_hash);
    write_u32_vec(&mut out, &classified.tok_types)?;
    write_u32_vec(&mut out, &classified.tok_starts)?;
    write_u32_vec(&mut out, &classified.tok_lens)?;
    write_u32_vec(&mut out, &classified.directive_kinds)?;
    out.extend_from_slice(
        &encode_len_u64(classified.source.len(), "classified source bytes")?.to_le_bytes(),
    );
    out.extend_from_slice(&classified.source);
    Ok(out)
}

fn reserve_encode_bytes(
    out: &mut Vec<u8>,
    additional: usize,
    field: &'static str,
) -> Result<(), String> {
    out.try_reserve_exact(additional).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {additional} bytes for {field}: {error:?}. Fix: shard C preprocess disk-cache encoding before serialization."
        )
    })
}

pub(crate) fn encode_len_u64(value: usize, field: &'static str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| {
        format!(
            "vyre-libs::gpu_pipeline: {field} length {value} exceeds u64 cache encoding range. Fix: shard C preprocess disk-cache encoding before serialization."
        )
    })
}

pub(crate) fn checked_encoded_add(
    total: &mut usize,
    additional: usize,
    field: &'static str,
) -> Result<(), String> {
    *total = total.checked_add(additional).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: {field} encoded byte length overflows usize. Fix: shard C preprocess disk-cache encoding before serialization."
        )
    })?;
    Ok(())
}

pub(crate) fn encoded_bytes_len(len: usize, field: &'static str) -> Result<usize, String> {
    let mut total = 8usize;
    checked_encoded_add(&mut total, len, field)?;
    Ok(total)
}

fn encoded_u32_vec_len(vec: &[u32], field: &'static str) -> Result<usize, String> {
    let payload = vec.len().checked_mul(4).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: {field} u32 payload length overflows usize. Fix: shard C preprocess disk-cache encoding before serialization."
        )
    })?;
    encoded_bytes_len(payload, field)
}

fn classified_encoded_len(
    path_bytes: &[u8],
    classified: &ClassifiedTokens,
) -> Result<usize, String> {
    let mut total = CLASSIFIED_DISK_MAGIC.len();
    checked_encoded_add(
        &mut total,
        encoded_bytes_len(path_bytes.len(), "classified path")?,
        "classified path",
    )?;
    checked_encoded_add(&mut total, 8, "classified source length")?;
    checked_encoded_add(&mut total, 16, "classified source hash")?;
    checked_encoded_add(
        &mut total,
        encoded_u32_vec_len(&classified.tok_types, "classified token types")?,
        "classified token types",
    )?;
    checked_encoded_add(
        &mut total,
        encoded_u32_vec_len(&classified.tok_starts, "classified token starts")?,
        "classified token starts",
    )?;
    checked_encoded_add(
        &mut total,
        encoded_u32_vec_len(&classified.tok_lens, "classified token lengths")?,
        "classified token lengths",
    )?;
    checked_encoded_add(
        &mut total,
        encoded_u32_vec_len(&classified.directive_kinds, "classified directive kinds")?,
        "classified directive kinds",
    )?;
    checked_encoded_add(
        &mut total,
        encoded_bytes_len(classified.source.len(), "classified source bytes")?,
        "classified source bytes",
    )?;
    Ok(total)
}

pub(crate) fn write_u32_vec(out: &mut Vec<u8>, vec: &[u32]) -> Result<(), String> {
    out.extend_from_slice(&encode_len_u64(vec.len(), "u32 vector")?.to_le_bytes());
    for value in vec {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum DecodeError {
    Truncated,
    BadMagic,
    KeyMismatch,
    Allocation { field: &'static str },
}

pub(crate) fn decode_len(value: u64) -> Result<usize, DecodeError> {
    usize::try_from(value).map_err(|_| DecodeError::Truncated)
}

pub(crate) fn reserve_decode_vec_capacity<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), DecodeError> {
    vec.try_reserve_exact(capacity)
        .map_err(|_| DecodeError::Allocation { field })
}

pub(crate) fn decode_classified(
    bytes: &[u8],
    expected_key: &ClassifiedCacheKey,
) -> Result<ClassifiedTokens, DecodeError> {
    let mut cursor = 0usize;
    if bytes.len() < CLASSIFIED_DISK_MAGIC.len()
        || &bytes[..CLASSIFIED_DISK_MAGIC.len()] != CLASSIFIED_DISK_MAGIC
    {
        return Err(DecodeError::BadMagic);
    }
    cursor += CLASSIFIED_DISK_MAGIC.len();
    let path_len = decode_len(read_u64(bytes, &mut cursor)?)?;
    let path_end = cursor.checked_add(path_len).ok_or(DecodeError::Truncated)?;
    if path_end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let path_str =
        std::str::from_utf8(&bytes[cursor..path_end]).map_err(|_| DecodeError::KeyMismatch)?;
    cursor = path_end;
    if std::path::Path::new(path_str) != expected_key.path.as_path() {
        return Err(DecodeError::KeyMismatch);
    }
    let source_len = decode_len(read_u64(bytes, &mut cursor)?)?;
    let source_hash = read_hash128(bytes, &mut cursor)?;
    if source_len != expected_key.source_len || source_hash != expected_key.source_hash {
        return Err(DecodeError::KeyMismatch);
    }
    let tok_types = read_u32_vec(bytes, &mut cursor)?;
    let tok_starts = read_u32_vec(bytes, &mut cursor)?;
    let tok_lens = read_u32_vec(bytes, &mut cursor)?;
    let directive_kinds = read_u32_vec(bytes, &mut cursor)?;
    let src_len = decode_len(read_u64(bytes, &mut cursor)?)?;
    let src_end = cursor.checked_add(src_len).ok_or(DecodeError::Truncated)?;
    if src_end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let source = std::sync::Arc::from(&bytes[cursor..src_end]);
    Ok(ClassifiedTokens::from_parts(
        tok_types,
        tok_starts,
        tok_lens,
        directive_kinds,
        source,
    ))
}

pub(crate) fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, DecodeError> {
    let end = cursor.checked_add(8).ok_or(DecodeError::Truncated)?;
    if end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(u64::from_le_bytes(buf))
}

pub(crate) fn read_hash128(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 16], DecodeError> {
    let end = cursor.checked_add(16).ok_or(DecodeError::Truncated)?;
    if end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(out)
}

pub(crate) fn read_u32_vec(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u32>, DecodeError> {
    let count = decode_len(read_u64(bytes, cursor)?)?;
    let span = count
        .checked_mul(4)
        .and_then(|n| cursor.checked_add(n))
        .ok_or(DecodeError::Truncated)?;
    if span > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let mut vec = Vec::new();
    reserve_decode_vec_capacity(&mut vec, count, "classified u32 vector")?;
    for _ in 0..count {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[*cursor..*cursor + 4]);
        vec.push(u32::from_le_bytes(buf));
        *cursor += 4;
    }
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_len_rejects_unrepresentable_wire_lengths() {
        if usize::BITS < u64::BITS {
            assert!(matches!(decode_len(u64::MAX), Err(DecodeError::Truncated)));
        } else {
            match decode_len(u64::MAX) {
                Ok(value) => assert_eq!(value, usize::MAX),
                Err(err) => panic!("u64::MAX must be representable as usize here: {err:?}"),
            }
        }
    }

    #[test]
    fn reserve_decode_vec_capacity_reports_capacity_overflow() {
        let mut values = Vec::<u8>::new();

        let err = reserve_decode_vec_capacity(&mut values, usize::MAX, "cache bytes")
            .expect_err("absurd decoded cache capacity must fail before allocation");

        assert!(matches!(
            err,
            DecodeError::Allocation {
                field: "cache bytes"
            }
        ));
        assert!(values.is_empty());
    }
}
