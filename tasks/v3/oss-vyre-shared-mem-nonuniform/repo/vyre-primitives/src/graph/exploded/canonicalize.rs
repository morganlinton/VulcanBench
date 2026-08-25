/// Sort each CSR row in place after validating row ranges.
pub fn canonicalize_csr_within_rows_in_place(
    row_ptr: &[u32],
    col_idx: &mut [u32],
) -> Result<(), String> {
    for window in row_ptr.windows(2) {
        let start = window[0] as usize;
        let end = window[1] as usize;
        if start > end || end > col_idx.len() {
            return Err(format!(
                "Fix: exploded IFDS CSR row range {start}..{end} exceeds col_idx.len()={}.",
                col_idx.len()
            ));
        }
        col_idx[start..end].sort_unstable();
    }
    Ok(())
}

/// Return a row-canonical CSR copy.
#[must_use]
pub fn canonicalize_csr_within_rows(row_ptr: &[u32], col_idx: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut canonical_col = col_idx.to_vec();
    // Falling back to the ORIGINAL (non-canonical) column index on failure
    // silently hands downstream code data it expects to be canonical — a silent
    // correctness fallback (Law 10). Fail loud; callers that can tolerate
    // malformed CSR call canonicalize_csr_within_rows_in_place directly.
    if let Err(error) = canonicalize_csr_within_rows_in_place(row_ptr, &mut canonical_col) {
        panic!("vyre-primitives CSR row canonicalization failed: {error}");
    }
    (row_ptr.to_vec(), canonical_col)
}
