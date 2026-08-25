//! Shared scratch-buffer initialization helpers for C preprocessing GPU filters.

pub(super) fn write_zero_bytes(
    out: &mut Vec<u8>,
    byte_len: usize,
    context: &str,
) -> Result<(), String> {
    write_fill_bytes(out, byte_len, 0, context)
}

pub(super) fn write_fill_bytes(
    out: &mut Vec<u8>,
    byte_len: usize,
    value: u8,
    context: &str,
) -> Result<(), String> {
    if out.capacity() < byte_len {
        out.try_reserve_exact(byte_len - out.capacity())
            .map_err(|e| {
                format!(
                    "{context}: could not reserve {byte_len} GPU filter scratch bytes. Fix: reduce batch size or increase host memory: {e}"
                )
            })?;
    }
    out.resize(byte_len, value);
    out.fill(value);
    Ok(())
}

pub(super) fn copy_output_bytes(bytes: &[u8], context: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len()).map_err(|e| {
        format!(
            "{context}: could not reserve {} output bytes. Fix: reduce batch size or increase host memory: {e}",
            bytes.len()
        )
    })?;
    out.extend_from_slice(bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_writer_reuses_capacity_and_overwrites_old_bytes() {
        let mut bytes = vec![0xFF; 16];
        let capacity = bytes.capacity();

        write_zero_bytes(&mut bytes, 8, "test zero")
            .expect("Fix: test scratch allocation should fit");

        assert_eq!(bytes, vec![0; 8]);
        assert!(
            bytes.capacity() >= capacity,
            "Fix: scratch zeroing must not shrink reusable allocation capacity."
        );
    }

    #[test]
    fn fill_writer_overwrites_existing_prefix_and_extension() {
        let mut bytes = vec![0; 4];

        write_fill_bytes(&mut bytes, 8, 0xFF, "test fill")
            .expect("Fix: test scratch allocation should fit");

        assert_eq!(bytes, vec![0xFF; 8]);
    }

    #[test]
    fn generated_copy_output_bytes_preserves_8192_prefix_shapes() {
        for len in 0..8192 {
            let input: Vec<u8> = (0..len)
                .map(|index| ((index * 131 + len * 17) & 0xFF) as u8)
                .collect();
            let copied = copy_output_bytes(&input, "generated copy")
                .expect("Fix: generated output copy should reserve exactly.");
            assert_eq!(copied, input);
        }
    }
}
