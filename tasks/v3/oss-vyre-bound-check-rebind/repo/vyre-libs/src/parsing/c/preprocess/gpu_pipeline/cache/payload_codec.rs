use super::super::DirectivePayload;
use super::classified_codec::{
    checked_encoded_add, decode_len, encode_len_u64, encoded_bytes_len, read_hash128, read_u64,
    reserve_decode_vec_capacity, DecodeError,
};
use super::payload_keys::{PayloadsCacheKey, PAYLOADS_DISK_MAGIC};

pub(crate) fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    out.extend_from_slice(&encode_len_u64(bytes.len(), "payload byte field")?.to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

pub(crate) fn read_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, DecodeError> {
    let len = decode_len(read_u64(bytes, cursor)?)?;
    let end = cursor.checked_add(len).ok_or(DecodeError::Truncated)?;
    if end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let mut value = Vec::new();
    reserve_decode_vec_capacity(&mut value, len, "payload byte field")?;
    value.extend_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(value)
}

pub(crate) fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, DecodeError> {
    let end = cursor.checked_add(4).ok_or(DecodeError::Truncated)?;
    if end > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(u32::from_le_bytes(buf))
}

pub(crate) fn encode_payload(out: &mut Vec<u8>, payload: &DirectivePayload) -> Result<(), String> {
    match payload {
        DirectivePayload::None => out.push(0),
        DirectivePayload::Define {
            name,
            name_start,
            name_len,
            args,
            args_start,
            args_len,
            body,
            body_start,
            body_len,
            is_function_like,
        } => {
            out.push(1);
            write_bytes(out, name)?;
            out.extend_from_slice(&name_start.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            write_bytes(out, args)?;
            out.extend_from_slice(&args_start.to_le_bytes());
            out.extend_from_slice(&args_len.to_le_bytes());
            write_bytes(out, body)?;
            out.extend_from_slice(&body_start.to_le_bytes());
            out.extend_from_slice(&body_len.to_le_bytes());
            out.push(if *is_function_like { 1 } else { 0 });
        }
        DirectivePayload::Undef { name } => {
            out.push(2);
            write_bytes(out, name)?;
        }
        DirectivePayload::Include {
            path,
            is_system,
            is_next,
        } => {
            out.push(3);
            write_bytes(out, path)?;
            out.push(if *is_system { 1 } else { 0 });
            out.push(if *is_next { 1 } else { 0 });
        }
        DirectivePayload::Ifdef { value, negated } => {
            out.push(4);
            out.extend_from_slice(&value.to_le_bytes());
            out.push(if *negated { 1 } else { 0 });
        }
        DirectivePayload::IfExpr { value, is_elif } => {
            out.push(5);
            out.extend_from_slice(&value.to_le_bytes());
            out.push(if *is_elif { 1 } else { 0 });
        }
        DirectivePayload::Else => out.push(6),
        DirectivePayload::Endif => out.push(7),
        DirectivePayload::Other => out.push(8),
    }
    Ok(())
}

pub(crate) fn decode_payload(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<DirectivePayload, DecodeError> {
    if *cursor >= bytes.len() {
        return Err(DecodeError::Truncated);
    }
    let tag = bytes[*cursor];
    *cursor += 1;
    match tag {
        0 => Ok(DirectivePayload::None),
        1 => {
            let name = read_bytes(bytes, cursor)?;
            let name_start = read_u32(bytes, cursor)?;
            let name_len = read_u32(bytes, cursor)?;
            let args = read_bytes(bytes, cursor)?;
            let args_start = read_u32(bytes, cursor)?;
            let args_len = read_u32(bytes, cursor)?;
            let body = read_bytes(bytes, cursor)?;
            let body_start = read_u32(bytes, cursor)?;
            let body_len = read_u32(bytes, cursor)?;
            if *cursor >= bytes.len() {
                return Err(DecodeError::Truncated);
            }
            let is_function_like = bytes[*cursor] != 0;
            *cursor += 1;
            Ok(DirectivePayload::Define {
                name,
                name_start,
                name_len,
                args,
                args_start,
                args_len,
                body,
                body_start,
                body_len,
                is_function_like,
            })
        }
        2 => Ok(DirectivePayload::Undef {
            name: read_bytes(bytes, cursor)?,
        }),
        3 => {
            let path = read_bytes(bytes, cursor)?;
            if cursor.checked_add(2).ok_or(DecodeError::Truncated)? > bytes.len() {
                return Err(DecodeError::Truncated);
            }
            let is_system = bytes[*cursor] != 0;
            let is_next = bytes[*cursor + 1] != 0;
            *cursor += 2;
            Ok(DirectivePayload::Include {
                path,
                is_system,
                is_next,
            })
        }
        4 => {
            let value = read_u32(bytes, cursor)?;
            if *cursor >= bytes.len() {
                return Err(DecodeError::Truncated);
            }
            let negated = bytes[*cursor] != 0;
            *cursor += 1;
            Ok(DirectivePayload::Ifdef { value, negated })
        }
        5 => {
            let value = read_u32(bytes, cursor)?;
            if *cursor >= bytes.len() {
                return Err(DecodeError::Truncated);
            }
            let is_elif = bytes[*cursor] != 0;
            *cursor += 1;
            Ok(DirectivePayload::IfExpr { value, is_elif })
        }
        6 => Ok(DirectivePayload::Else),
        7 => Ok(DirectivePayload::Endif),
        8 => Ok(DirectivePayload::Other),
        _ => Err(DecodeError::BadMagic),
    }
}

pub(crate) fn encode_payloads(
    key: &PayloadsCacheKey,
    payloads: &[DirectivePayload],
) -> Result<Vec<u8>, String> {
    let path_bytes = key
        .path
        .as_os_str()
        .to_string_lossy()
        .into_owned()
        .into_bytes();
    let encoded_len = payloads_encoded_len(&path_bytes, payloads)?;
    let mut out = Vec::new();
    out.try_reserve_exact(encoded_len).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {encoded_len} bytes for payload cache entry: {error:?}. Fix: shard C preprocess payload-cache encoding before serialization."
        )
    })?;
    out.extend_from_slice(PAYLOADS_DISK_MAGIC);
    write_bytes(&mut out, &path_bytes)?;
    out.extend_from_slice(&encode_len_u64(key.source_len, "payload source length")?.to_le_bytes());
    out.extend_from_slice(&key.source_hash);
    out.extend_from_slice(&key.macro_fingerprint);
    out.extend_from_slice(&encode_len_u64(payloads.len(), "payload count")?.to_le_bytes());
    for payload in payloads {
        encode_payload(&mut out, payload)?;
    }
    Ok(out)
}

fn payloads_encoded_len(path_bytes: &[u8], payloads: &[DirectivePayload]) -> Result<usize, String> {
    let mut total = PAYLOADS_DISK_MAGIC.len();
    checked_encoded_add(
        &mut total,
        encoded_bytes_len(path_bytes.len(), "payload cache path")?,
        "payload cache path",
    )?;
    checked_encoded_add(&mut total, 8, "payload source length")?;
    checked_encoded_add(&mut total, 16, "payload source hash")?;
    checked_encoded_add(&mut total, 16, "payload macro fingerprint")?;
    checked_encoded_add(&mut total, 8, "payload count")?;
    for payload in payloads {
        let payload_len = payload_encoded_len(payload)?;
        checked_encoded_add(&mut total, payload_len, "payload body")?;
    }
    Ok(total)
}

fn payload_encoded_len(payload: &DirectivePayload) -> Result<usize, String> {
    match payload {
        DirectivePayload::None
        | DirectivePayload::Else
        | DirectivePayload::Endif
        | DirectivePayload::Other => Ok(1),
        DirectivePayload::Define {
            name, args, body, ..
        } => {
            let mut total = 1usize;
            checked_encoded_add(
                &mut total,
                encoded_bytes_len(name.len(), "define payload name")?,
                "define payload name",
            )?;
            checked_encoded_add(&mut total, 8, "define payload name span")?;
            checked_encoded_add(
                &mut total,
                encoded_bytes_len(args.len(), "define payload args")?,
                "define payload args",
            )?;
            checked_encoded_add(&mut total, 8, "define payload args span")?;
            checked_encoded_add(
                &mut total,
                encoded_bytes_len(body.len(), "define payload body")?,
                "define payload body",
            )?;
            checked_encoded_add(&mut total, 9, "define payload body span and kind")?;
            Ok(total)
        }
        DirectivePayload::Undef { name } => {
            let mut total = 1usize;
            checked_encoded_add(
                &mut total,
                encoded_bytes_len(name.len(), "undef payload name")?,
                "undef payload name",
            )?;
            Ok(total)
        }
        DirectivePayload::Include { path, .. } => {
            let mut total = 1usize;
            checked_encoded_add(
                &mut total,
                encoded_bytes_len(path.len(), "include payload path")?,
                "include payload path",
            )?;
            checked_encoded_add(&mut total, 2, "include payload flags")?;
            Ok(total)
        }
        DirectivePayload::Ifdef { .. } | DirectivePayload::IfExpr { .. } => Ok(6),
    }
}

pub(crate) fn decode_payloads(
    bytes: &[u8],
    expected_key: &PayloadsCacheKey,
) -> Result<Vec<DirectivePayload>, DecodeError> {
    let mut cursor = 0usize;
    if bytes.len() < PAYLOADS_DISK_MAGIC.len()
        || &bytes[..PAYLOADS_DISK_MAGIC.len()] != PAYLOADS_DISK_MAGIC
    {
        return Err(DecodeError::BadMagic);
    }
    cursor += PAYLOADS_DISK_MAGIC.len();
    let path_bytes = read_bytes(bytes, &mut cursor)?;
    let path_str = std::str::from_utf8(&path_bytes).map_err(|_| DecodeError::KeyMismatch)?;
    if std::path::Path::new(path_str) != expected_key.path.as_path() {
        return Err(DecodeError::KeyMismatch);
    }
    let source_len = decode_len(read_u64(bytes, &mut cursor)?)?;
    let source_hash = read_hash128(bytes, &mut cursor)?;
    let macro_fingerprint = read_hash128(bytes, &mut cursor)?;
    if source_len != expected_key.source_len
        || source_hash != expected_key.source_hash
        || macro_fingerprint != expected_key.macro_fingerprint
    {
        return Err(DecodeError::KeyMismatch);
    }
    let count = decode_len(read_u64(bytes, &mut cursor)?)?;
    let mut payloads = Vec::new();
    reserve_decode_vec_capacity(&mut payloads, count, "directive payload cache entries")?;
    for _ in 0..count {
        payloads.push(decode_payload(bytes, &mut cursor)?);
    }
    Ok(payloads)
}

#[cfg(test)]
mod payload_codec_allocation_tests {
    use super::*;

    #[test]
    fn read_bytes_reserves_fallibly_before_copying_payload() {
        let mut encoded = Vec::new();
        write_bytes(&mut encoded, b"payload").expect("Fix: codec write failures indicate corrupt buffer state; abort encode and return Err - payload byte field should encode");
        let mut cursor = 0usize;

        let decoded = read_bytes(&encoded, &mut cursor).expect("Fix: reject truncated or hostile codec payloads; do not decode past bounds - encoded bytes must decode");

        assert_eq!(decoded, b"payload");
        assert_eq!(cursor, encoded.len());
    }

    #[test]
    fn read_bytes_rejects_absurd_length_without_allocating() {
        let mut encoded = u64::MAX.to_le_bytes().to_vec();
        encoded.extend_from_slice(b"x");
        let mut cursor = 0usize;

        let err = read_bytes(&encoded, &mut cursor)
            .expect_err("absurd payload byte length must not allocate");

        assert!(matches!(err, DecodeError::Truncated));
        assert_eq!(cursor, 8);
    }
}
