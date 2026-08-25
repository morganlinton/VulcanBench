//! Contract tests for lattice/semiring algebra and diversity sketches.
//!
//! Covers: lattice_join, lattice_meet, semiring_min_plus_mul, sketch_mix.
//! Properties tested: specific value correctness, algebraic laws,
//! boundary behaviour (size-0, size-1, all-ones, all-zeros), saturation,
//! and builder error paths (aliasing names).
//!
//! GPU acquisition: none  -  every test routes through the reference
//! interpreter or Reference oracle paths only.

#![cfg(feature = "math-algebra")]
#![allow(deprecated)]
mod common;
use common::{decode_u32_words, u32_bytes};
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Lattice Join (bitwise OR)
// ---------------------------------------------------------------------------

mod algebra_lattice_semiring_contracts_part1 {

    include!("__split/algebra_lattice_semiring_contracts_part1.rs");
}
mod algebra_lattice_semiring_contracts_part2 {
    include!("__split/algebra_lattice_semiring_contracts_part2.rs");
}
