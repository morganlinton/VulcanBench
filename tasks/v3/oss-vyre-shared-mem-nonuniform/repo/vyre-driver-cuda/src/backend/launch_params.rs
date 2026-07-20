//! Checked CUDA launch-parameter sizing.

use vyre_driver::BackendError;

/// Return the byte length of a CUDA launch-parameter word block.
pub(crate) fn launch_param_byte_len(
    param_words: &[u32],
    context: &'static str,
) -> Result<usize, BackendError> {
    param_words
        .len()
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA {context} launch-parameter byte count overflowed usize for {} u32 word(s); split the parameter block before launch.",
                param_words.len()
            ),
        })
}
