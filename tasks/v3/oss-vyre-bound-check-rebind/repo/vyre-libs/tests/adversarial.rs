//! Hostile-input and edge-case tests for vyre-libs (`.internals/skills/testing` **adversarial** category).
//!
//! Full coverage is split across focused binaries for faster iteration:
//! - [`f32_adversarial`](./f32_adversarial.rs)  -  float edge cases
//! - [`op_boundaries`](./op_boundaries.rs)  -  op argument bounds
//! - [`overflow_guards`](./overflow_guards.rs)  -  numeric wrap / reject paths
//!
//! Run the suite:
//! `cargo test -p vyre-libs --test adversarial --test f32_adversarial --test op_boundaries --test overflow_guards`
//!
//! This file is the canonical `--test adversarial` entry so `tests/SKILL.md` and
//! `../../.internals/skills/testing/SKILL.md` align with a named binary.

#[test]
fn adversarial_test_layout_smoke() {
    // If this test runs, the adversarial test binary is wired. Keep a trivial
    // pass so `cargo test -p vyre-libs --test adversarial` is green; real
    // adversarial cases live in the modules above.
    assert_eq!(0_u32, 0_u32);
}
