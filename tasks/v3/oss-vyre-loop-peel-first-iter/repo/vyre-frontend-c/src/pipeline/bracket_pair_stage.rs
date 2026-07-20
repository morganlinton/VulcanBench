use super::*;
pub(super) fn c11_dual_bracket_pairs_cost_model(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    label: &str,
) -> Result<(Vec<u32>, Vec<u32>), String> {
    dispatch_c11_bracket_pairs(backend, tok_types, label)
}
