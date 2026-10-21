#[derive(Debug, Clone, Copy)]
pub(super) struct ConditionalFrame {
    /// `true` if the parent stack frame was active when this frame was pushed.
    pub(super) parent_active: bool,
    /// `true` if any branch in this `#if/#elif/#else` chain has been taken.
    pub(super) branch_taken: bool,
    /// Computed active state for the current branch.
    pub(super) current_active: bool,
    /// `true` after this conditional chain has consumed its `#else`.
    pub(super) saw_else: bool,
}
