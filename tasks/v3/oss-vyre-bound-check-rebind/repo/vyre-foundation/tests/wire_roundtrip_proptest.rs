//! Wire-format roundtrip proptest. Implementation is split across one
//! support file (chunk1+chunk2) plus part1 and part2; the support file
//! opens a `proptest!` block whose body and closing brace span across
//! part1  -  keep these in one flat scope rather than wrapping each
//! include in a sub-module.
#![allow(dead_code)]
mod wire_roundtrip_proptest_suite {
    include!("__split/wire_roundtrip_proptest_support_chunk1.rs");
    include!("__split/wire_roundtrip_proptest_support_chunk2.rs");
    include!("__split/wire_roundtrip_proptest_part1.rs");
    include!("__split/wire_roundtrip_proptest_part2.rs");
}
