use super::MegakernelWorkItem;

/// ROADMAP C3 substrate: shared prologue extraction.
///
/// Returns the length of the longest identical prefix shared by every
/// item in `arm_prologues`. The dispatcher uses this to extract a
/// single shared prologue at the top of the megakernel instead of
/// running the same prologue ops once per arm  -  saves N-1 redundant
/// prologue executions when N arms agree on the first K ops.
///
/// Returns `0` when:
///   - `arm_prologues` is empty,
///   - any prologue is empty (no shared work to extract), or
///   - the very first MegakernelWorkItem differs across arms.
///
/// `MegakernelWorkItem` derives `PartialEq` so equality is structural over
/// `(op_handle, input_handle, output_handle, param)`.
#[must_use]
pub fn shared_prologue_length(arm_prologues: &[&[MegakernelWorkItem]]) -> usize {
    if arm_prologues.is_empty() {
        return 0;
    }
    let min_len = arm_prologues
        .iter()
        .map(|prologue| prologue.len())
        .min()
        .unwrap_or(0);
    let first = arm_prologues[0];
    let mut shared = 0_usize;
    while shared < min_len {
        let candidate = first[shared];
        if arm_prologues
            .iter()
            .all(|prologue| prologue[shared] == candidate)
        {
            shared += 1;
        } else {
            break;
        }
    }
    shared
}
