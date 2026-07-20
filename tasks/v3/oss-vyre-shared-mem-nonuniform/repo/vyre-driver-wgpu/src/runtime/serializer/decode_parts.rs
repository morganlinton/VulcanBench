use super::encode_parts::{MAGIC, MAX_PART_COUNT, MAX_SERIALIZED_PART_BYTES};
use vyre_driver::error::{Error, Result};

/// Decode a blob produced by `super::encode_parts`.
///
/// # Errors
///
/// Returns an actionable error when the frame header, length prefixes, or
/// declared payload sizes are invalid for this platform.
#[inline]
pub fn decode_parts(mut bytes: &[u8]) -> Result<Vec<&[u8]>> {
    if bytes.len() < MAGIC.len() || bytes[..MAGIC.len()] != MAGIC {
        return Err(Error::Gpu {
            message: "invalid vyre serializer header. Fix: pass data produced by encode_parts without trimming or prefixing bytes.".to_string(),
        });
    }
    bytes = &bytes[MAGIC.len()..];
    let part_count = validated_part_count(bytes)?;
    let mut parts = Vec::new();
    parts
        .try_reserve_exact(part_count)
        .map_err(|source| Error::Serialization {
            message: format!(
                "could not reserve {part_count} framed part slots exactly: {source}. Fix: split the frame into fewer parts before decoding."
            ),
        })?;
    while !bytes.is_empty() {
        let len = decode_part_len(bytes)?;
        bytes = &bytes[8..];
        let (part, rest) = bytes.split_at(len);
        parts.push(part);
        bytes = rest;
    }
    Ok(parts)
}

fn validated_part_count(mut bytes: &[u8]) -> Result<usize> {
    let mut count = 0usize;
    while !bytes.is_empty() {
        if count == MAX_PART_COUNT {
            return Err(Error::Serialization {
                message: format!(
                    "framed part count exceeds {MAX_PART_COUNT}. Fix: reject this frame or split the payload before framing."
                ),
            });
        }
        let len = decode_part_len(bytes)?;
        bytes = &bytes[8..];
        if bytes.len() < len {
            return Err(Error::Gpu {
                message: "truncated framed part payload. Fix: provide the full payload declared by the preceding length.".to_string(),
            });
        }
        bytes = &bytes[len..];
        count += 1;
    }
    Ok(count)
}

fn decode_part_len(bytes: &[u8]) -> Result<usize> {
    if bytes.len() < 8 {
        return Err(Error::Gpu {
            message: "truncated framed part length. Fix: provide all 8 bytes of each encoded part length."
                .to_string(),
        });
    }
    let raw_len =
        u64::from_le_bytes(bytes[..8].try_into().map_err(|source| Error::Serialization {
            message: format!("invalid framed part length: {source}. Fix: provide an intact 8-byte little-endian part length."),
        })?);
    let len = usize::try_from(raw_len).map_err(|source| Error::Serialization {
        message: format!("SerializationOverflow: framed part length {raw_len} cannot fit usize: {source}. Fix: reject this frame on this platform or split the payload."),
    })?;
    if len > MAX_SERIALIZED_PART_BYTES {
        return Err(Error::Serialization {
            message: format!(
                "framed part declares {len} bytes, exceeding {MAX_SERIALIZED_PART_BYTES}. Fix: reject this frame or split the payload before framing."
            ),
        });
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::serializer::encode_parts;

    #[test]
    fn decode_reserves_exact_declared_part_count_not_payload_sized_guess() {
        let payload = [7u8; 128];
        let encoded = encode_parts(&[payload.as_slice()]).expect("Fix: frame should encode");
        let decoded = decode_parts(&encoded).expect("Fix: frame should decode");

        assert_eq!(decoded, vec![payload.as_slice()]);
        assert_eq!(
            decoded.capacity(),
            1,
            "Fix: decode_parts must reserve by declared part count, not bytes.len() / 8"
        );
    }

    #[test]
    fn validated_part_count_rejects_truncated_payload_before_allocation() {
        let mut encoded = Vec::from(MAGIC);
        encoded.extend_from_slice(&(4u64).to_le_bytes());
        encoded.extend_from_slice(&[1, 2, 3]);

        let error = decode_parts(&encoded).expect_err("truncated payload must fail");
        assert!(
            error.to_string().contains("truncated framed part payload"),
            "Fix: truncated payload must be rejected during validation before output allocation: {error}"
        );
    }
}
