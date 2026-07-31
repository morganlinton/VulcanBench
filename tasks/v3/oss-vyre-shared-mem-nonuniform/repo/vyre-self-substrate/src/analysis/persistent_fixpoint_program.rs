//! Persistent-fixpoint Program builder for runtime and driver scheduling loops.

use vyre_foundation::ir::{Node, Program};

/// Build a persistent-fixpoint Program around a caller-supplied transfer body.
///
/// The generated program runs `transfer_body`, ping-pongs `current` and `next`,
/// and stops when `changed[0] == 0` or `max_iterations` is reached. Runtime and
/// driver crates call this self-substrate wrapper instead of depending on the
/// primitive catalog directly.
#[must_use]
pub fn persistent_fixpoint_program(
    transfer_body: Vec<Node>,
    current: &str,
    next: &str,
    changed: &str,
    words: u32,
    max_iterations: u32,
) -> Program {
    vyre_primitives::fixpoint::persistent_fixpoint::persistent_fixpoint(
        transfer_body,
        current,
        next,
        changed,
        words,
        max_iterations,
    )
}

#[cfg(test)]
mod tests {
    use super::persistent_fixpoint_program;

    #[test]
    fn builds_program_with_caller_buffers() {
        let program = persistent_fixpoint_program(Vec::new(), "current", "next", "changed", 4, 8);
        let names = program
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();

        assert!(names.contains(&"current"));
        assert!(names.contains(&"next"));
        assert!(names.contains(&"changed"));
    }
}
