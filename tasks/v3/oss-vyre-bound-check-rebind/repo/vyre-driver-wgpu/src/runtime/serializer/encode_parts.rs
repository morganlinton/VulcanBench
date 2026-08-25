use vyre_driver::error::{Error, Result};

pub(crate) const MAGIC: [u8; 8] = *b"VYREPART";

/// Maximum number of parts accepted by the runtime framing layer.
///
/// Zero-length parts are valid but still consume metadata and returned slice
/// slots. This cap prevents adversarial frames from allocating unbounded part
/// tables with tiny payloads.
pub(crate) const MAX_PART_COUNT: usize = 1_048_576;

/// Maximum size of one serialized part accepted by the runtime framing layer.
///
/// I10: this rejects a single oversized part before total frame sizing and
/// fallible output reservation. The 256 MiB cap matches bounded GPU payload
/// staging and keeps malformed frames from driving unbounded host allocation.
/// Prevents OOM by capping memory used during serialization.
pub const MAX_SERIALIZED_PART_BYTES: usize = 256 * 1024 * 1024;

/// Frame multiple byte slices into a single blob with length prefixes.
///
/// # Errors
///
/// Returns an error if the combined frame length cannot be represented on this
/// platform.
///
/// # Examples
///
/// ```
/// use vyre_driver_wgpu::runtime::serializer::{encode_parts, decode_parts};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let parts = vec![b"abc".as_slice(), b"def".as_slice()];
/// let encoded = encode_parts(&parts)?;
/// let decoded = decode_parts(&encoded)?;
/// assert_eq!(decoded, parts);
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn encode_parts(parts: &[&[u8]]) -> Result<Vec<u8>> {
    if parts.len() > MAX_PART_COUNT {
        return Err(Error::Serialization {
            message: format!(
                "serialized frame has {} parts, exceeding {MAX_PART_COUNT}. Fix: split the frame into fewer parts before encoding.",
                parts.len()
            ),
        });
    }
    for part in parts {
        if part.len() > MAX_SERIALIZED_PART_BYTES {
            return Err(Error::Serialization {
                message: format!(
                    "serialized part is {} bytes, exceeding {MAX_SERIALIZED_PART_BYTES}. Fix: split the payload into smaller parts before framing.",
                    part.len()
                ),
            });
        }
    }
    let payload_len = parts.iter().try_fold(0usize, |sum, part| {
        sum.checked_add(8)
            .and_then(|value| value.checked_add(part.len()))
            .ok_or_else(|| Error::Serialization {
                message: "SerializationOverflow: framed part size calculation overflow. Fix: split the payload into smaller encode_parts calls.".to_string(),
            })
    })?;
    let total = MAGIC.len().checked_add(payload_len).ok_or_else(|| Error::Serialization {
        message: "SerializationOverflow: framed output size calculation overflow. Fix: split the payload into smaller encode_parts calls.".to_string(),
    })?;
    let mut out = Vec::new();
    reserve_encoded_frame(&mut out, total)?;
    out.extend_from_slice(&MAGIC);
    for part in parts {
        let len = u64::try_from(part.len()).map_err(|source| Error::Serialization {
            message: format!("framed part length {} exceeds u64::MAX: {source}. Fix: split the payload before framing.", part.len()),
        })?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(part);
    }
    Ok(out)
}

fn reserve_encoded_frame(out: &mut Vec<u8>, total: usize) -> Result<()> {
    out.try_reserve_exact(total).map_err(|source| Error::Serialization {
        message: format!(
            "could not reserve {total} encoded frame bytes exactly: {source}. Fix: split the payload into smaller encode_parts calls."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_parts_uses_fallible_exact_reservation() {
        let production = include_str!("encode_parts.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: serializer encoder production section should precede tests");

        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: encode_parts must not use infallible frame allocation."
        );
        assert!(
            production.contains("try_reserve_exact(total)"),
            "Fix: encode_parts should reserve the validated frame length fallibly before appending."
        );
    }

    #[test]
    fn encode_parts_capacity_matches_validated_frame_size() {
        let parts = [b"abc".as_slice(), b"defgh".as_slice()];
        let encoded = encode_parts(&parts).expect("Fix: small frame should encode");
        let expected = MAGIC.len() + 8 + 3 + 8 + 5;

        assert_eq!(encoded.len(), expected);
        assert_eq!(
            encoded.capacity(),
            expected,
            "Fix: encode_parts should reserve exactly the validated frame length."
        );
    }
}
