use super::*;

pub(crate) fn checked_staging_word_bytes(word_count: usize, label: &str) -> Result<usize, String> {
    word_count.checked_mul(4).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: {label} byte length overflowed. Fix: shard macro expansion staging before GPU dispatch."
        )
    })
}

pub(crate) fn reserve_staging_bytes(
    out: &mut Vec<u8>,
    byte_len: usize,
    label: &str,
) -> Result<(), String> {
    out.try_reserve_exact(byte_len).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: failed to reserve {byte_len} bytes for {label}: {error}. Fix: shard macro expansion staging before GPU dispatch."
        )
    })
}

pub(crate) fn materialized_output_program(program: Program) -> Program {
    let buffers = program
        .buffers()
        .iter()
        .cloned()
        .map(|mut buffer| {
            if (17..=22).contains(&buffer.binding) {
                buffer.access = BufferAccess::WriteOnly;
                buffer.pipeline_live_out = true;
            }
            buffer
        })
        .collect::<Vec<_>>();
    program.with_rewritten_buffers(buffers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_expansion_staging_reports_word_byte_overflow() {
        let err = checked_staging_word_bytes(usize::MAX, "generated staging")
            .expect_err("absurd pad length must fail before staging allocation");

        assert!(err.contains("byte length overflowed"), "{err}");
    }

    #[test]
    fn macro_expansion_staging_reports_reservation_failure() {
        let mut out = Vec::new();

        let err = reserve_staging_bytes(&mut out, usize::MAX, "generated staging")
            .expect_err("absurd staging allocation must be reported");

        assert!(err.contains("failed to reserve"), "{err}");
        assert!(out.is_empty());
    }
}
