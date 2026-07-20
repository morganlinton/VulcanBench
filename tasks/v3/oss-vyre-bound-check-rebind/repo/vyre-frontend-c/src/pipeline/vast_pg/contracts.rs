#[cfg(test)]
pub(super) fn take_exact_stage_outputs(
    stage: &str,
    outputs: Vec<Vec<u8>>,
    expected_count: usize,
) -> Result<Vec<Vec<u8>>, String> {
    if outputs.len() != expected_count {
        return Err(format!(
            "{stage}: expected exactly {expected_count} output buffer(s), got {}. Fix: backend must return the declared stage ABI outputs and no extras.",
            outputs.len()
        ));
    }
    Ok(outputs)
}

pub(super) fn require_exact_readback_bytes(
    stage: &str,
    output: &str,
    blob: &[u8],
    expected_byte_len: u64,
    readback: bool,
) -> Result<(), String> {
    if !readback {
        return Ok(());
    }
    let expected_len = usize::try_from(expected_byte_len).map_err(|_| {
        format!(
            "{stage}: expected byte length {expected_byte_len} for {output} exceeds this platform's addressable memory. Fix: split the stage into bounded chunks before dispatch."
        )
    })?;
    if blob.len() != expected_len {
        return Err(format!(
            "{stage}: malformed {output} readback: expected {expected_len} bytes, got {}. Fix: backend must materialize the exact declared artifact size.",
            blob.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{require_exact_readback_bytes, take_exact_stage_outputs};

    #[test]
    fn exact_stage_outputs_rejects_extra_buffers() {
        let err = take_exact_stage_outputs("stage", vec![vec![0], vec![1]], 1).unwrap_err();
        assert!(err.contains("expected exactly 1 output buffer"));
    }

    #[test]
    fn readback_size_contract_accepts_empty_non_readback_artifact() {
        require_exact_readback_bytes("stage", "out", &[], 64, false).unwrap();
    }

    #[test]
    fn readback_size_contract_rejects_truncated_artifact() {
        let err = require_exact_readback_bytes("stage", "out", &[0; 12], 16, true).unwrap_err();
        assert!(err.contains("malformed out readback"));
    }
}
