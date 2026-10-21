//! Property gates for `vyre_primitives::bitset::and::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::and::cpu_ref;

#[macro_use]
mod bitset_law_support;

bitset_and_law_tests!(cpu_ref);
