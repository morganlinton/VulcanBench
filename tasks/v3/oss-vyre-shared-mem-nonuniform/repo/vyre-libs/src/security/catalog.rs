use crate::harness::OpEntry;

fn u32s(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

macro_rules! bitset_and_entry {
    ($module:ident, $build:expr) => {
        inventory::submit! {
            OpEntry {
                id: super::$module::OP_ID,
                build: $build,
                test_inputs: Some(|| {
                    super::predicate_catalog::packed_witness_inputs(super::$module::OP_ID)
                }),
                expected_output: Some(|| {
                    super::predicate_catalog::packed_witness_expected(super::$module::OP_ID)
                }),
                category: Some("security"),
            }
        }
    };
}

macro_rules! bitset_and_not_entry {
    ($module:ident, $build:expr) => {
        inventory::submit! {
            OpEntry {
                id: super::$module::OP_ID,
                build: $build,
                test_inputs: Some(|| {
                    super::predicate_catalog::packed_witness_inputs(super::$module::OP_ID)
                }),
                expected_output: Some(|| {
                    super::predicate_catalog::packed_witness_expected(super::$module::OP_ID)
                }),
                category: Some("security"),
            }
        }
    };
}

bitset_and_entry!(auth_check_dominates, || {
    super::auth_check_dominates::auth_check_dominates(4, "a", "b", "out")
});
bitset_and_entry!(buffer_size_check, || {
    super::buffer_size_check::buffer_size_check(4, "a", "b", "out")
});
bitset_and_entry!(lock_dominates, || {
    super::lock_dominates::lock_dominates(4, "a", "b", "out")
});
bitset_and_entry!(path_canonical, || {
    super::path_canonical::path_canonical(4, "a", "b", "out")
});
bitset_and_entry!(sanitizer_dominates, || {
    super::sanitizer_dominates::sanitizer_dominates(4, "a", "b", "out")
});
bitset_and_entry!(sql_param_bound, || {
    super::sql_param_bound::sql_param_bound(4, "a", "b", "out")
});
bitset_and_entry!(xss_escape, || {
    super::xss_escape::xss_escape(4, "a", "b", "out")
});

bitset_and_not_entry!(format_string_check, || {
    super::format_string_check::format_string_check(4, "a", "b", "out")
});
bitset_and_not_entry!(taint_kill, || {
    super::taint_kill::taint_kill(4, "a", "b", "out")
});
bitset_and_not_entry!(unchecked_return, || {
    super::unchecked_return::unchecked_return(4, "a", "b", "out")
});

inventory::submit! {
    OpEntry {
        id: super::sink_intersection::OP_ID,
        build: || super::sink_intersection::sink_intersection(4, "a", "b", "scratch", "out"),
        test_inputs: Some(|| vec![vec![
            u32s(&[0b1100]),
            u32s(&[0b1010]),
            u32s(&[0]),
            u32s(&[0]),
        ]]),
        expected_output: Some(|| vec![vec![
            u32s(&[0b1000]),
            u32s(&[1]),
        ]]),
        category: Some("security"),
    }
}

inventory::submit! {
    OpEntry {
        id: super::integer_overflow_arith::OP_ID,
        build: || {
            super::integer_overflow_arith::integer_overflow_arith(
                4, "arith", "reach", "guards", "scratch", "out",
            )
        },
        test_inputs: Some(|| vec![vec![
            u32s(&[0b1111]),
            u32s(&[0b1100]),
            u32s(&[0b1000]),
            u32s(&[0]),
            u32s(&[0]),
        ]]),
        expected_output: Some(|| vec![vec![
            u32s(&[0b1100]),
            u32s(&[0b0100]),
        ]]),
        category: Some("security"),
    }
}
