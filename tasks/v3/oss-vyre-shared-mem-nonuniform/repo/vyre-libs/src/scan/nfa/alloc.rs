//! Checked allocation helpers shared by NFA compiler submodules.

use super::NfaCompileError;

pub(super) fn reserve_vec<T>(
    vec: &mut Vec<T>,
    requested: usize,
    field: &'static str,
) -> Result<(), NfaCompileError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(vec, requested).map_err(|source| {
        NfaCompileError::StorageReserveFailed {
            field,
            requested,
            message: source.to_string(),
        }
    })
}
