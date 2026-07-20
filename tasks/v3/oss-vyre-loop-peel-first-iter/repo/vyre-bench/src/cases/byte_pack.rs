use crate::api::case::BenchError;

pub(crate) use vyre_primitives::wire::pack_f32_slice as f32_bytes;

pub(crate) use vyre_primitives::wire::pack_u32_slice as u32_bytes;

pub(crate) fn decode_u64_words(bytes: &[u8], context: &str) -> Result<Vec<u64>, BenchError> {
    if bytes.len() % 8 != 0 {
        return Err(BenchError::CorrectnessViolation(format!(
            "{context} metric payload length {} is not divisible by 8",
            bytes.len()
        )));
    }
    Ok(vyre_primitives::wire::decode_u64_le_bytes_all(bytes))
}
