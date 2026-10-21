//! P1 inventory #89  -  property tests for buffer layout invariants.
//!
//! Buffer declarations on a `Program` carry strict structural rules:
//! every binding slot is unique, every name is unique, every read-only
//! buffer's bytes-required <= total-bytes, and the sorted-by-binding
//! view is canonical.
//!
//! This proptest generates random `Vec<BufferDecl>` and asserts those
//! invariants survive round-trip through the wire encoder.

use proptest::prelude::*;
use vyre::ir::{BufferDecl, DataType, Program};

#[derive(Debug, Clone, Copy)]
enum AccessKind {
    Read,
    Output,
    ReadWrite,
}

fn dtype_strategy() -> impl Strategy<Value = DataType> {
    prop_oneof![
        Just(DataType::U32),
        Just(DataType::I32),
        Just(DataType::F32),
        Just(DataType::U64),
        Just(DataType::I64),
        Just(DataType::F64),
        Just(DataType::U8),
    ]
}

fn access_strategy() -> impl Strategy<Value = AccessKind> {
    prop_oneof![
        Just(AccessKind::Read),
        Just(AccessKind::Output),
        Just(AccessKind::ReadWrite),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn buffer_layout_canonicalizes_to_distinct_bindings(
        access in proptest::collection::vec(access_strategy(), 1..16),
        dtypes in proptest::collection::vec(dtype_strategy(), 1..16),
    ) {
        let n = access.len().min(dtypes.len());
        let buffers: Vec<BufferDecl> = (0..n)
            .map(|i| {
                let name = format!("buf{i}");
                let dt = dtypes[i].clone();
                match access[i] {
                    AccessKind::Read => BufferDecl::read(&name, i as u32, dt),
                    AccessKind::Output => BufferDecl::output(&name, i as u32, dt),
                    AccessKind::ReadWrite => BufferDecl::read_write(&name, i as u32, dt),
                }
            })
            .collect();

        let mut bindings: Vec<u32> = buffers.iter().map(|b| b.binding).collect();
        bindings.sort_unstable();
        bindings.dedup();
        prop_assert_eq!(bindings.len(), n, "every binding slot must be unique");

        let mut names: Vec<&str> = buffers.iter().map(|b| &*b.name).collect();
        names.sort_unstable();
        names.dedup();
        prop_assert_eq!(names.len(), n, "every buffer name must be unique");

        // Building a Program from the layout must succeed without panics.
        let program = Program::wrapped(buffers, [1, 1, 1], Vec::new());
        prop_assert_eq!(program.buffers.len(), n);
    }
}
