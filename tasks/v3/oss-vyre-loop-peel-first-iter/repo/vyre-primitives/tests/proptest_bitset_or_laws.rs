//! Property gates for `vyre_primitives::bitset::or::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::or::cpu_ref;

#[macro_use]
mod bitset_law_support;

bitset_or_law_tests!(cpu_ref);
