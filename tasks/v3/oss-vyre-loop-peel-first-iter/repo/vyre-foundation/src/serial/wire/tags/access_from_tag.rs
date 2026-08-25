use crate::ir::BufferAccess;

/// Decode a [`BufferAccess`] from its VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `tag` must be a tag assigned by the VIR0 specification. Values outside
/// the defined tag space indicate a wire-format version mismatch or a
/// truncated/malicious blob.
///
/// # Returns
///
/// `Ok(BufferAccess)` on a recognized tag. The mapping is stable:
/// `0 → ReadOnly`, `1 → ReadWrite`, `2 → Uniform`, `3 → Workgroup`,
/// `4 → WriteOnly`.
///
/// # Failure mode
///
/// Returns `Err("Fix: unknown buffer access tag {tag}; use a compatible IR serializer.")`
/// for any unrecognized tag so callers reject the blob with an actionable
/// diagnostic instead of panicking or defaulting.
#[inline]
pub(crate) fn access_from_tag(tag: u8) -> Result<BufferAccess, String> {
    match tag {
        0 => Ok(BufferAccess::ReadOnly),
        1 => Ok(BufferAccess::ReadWrite),
        2 => Ok(BufferAccess::Uniform),
        3 => Ok(BufferAccess::Workgroup),
        4 => Ok(BufferAccess::WriteOnly),
        _ => Err(format!(
            "Fix: unknown buffer access tag {tag}; use a compatible IR serializer."
        )),
    }
}
