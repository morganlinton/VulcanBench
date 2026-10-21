//! Property gates for bytecode dispatch table pack/unpack round-trip.

#![cfg(feature = "parsing")]

use proptest::prelude::*;
use vyre_primitives::parsing::bytecode_dispatch_table_pack::{
    pack_dispatch_table, unpack_entry, OpcodeHandlerEntry,
};

fn arb_entry() -> impl Strategy<Value = OpcodeHandlerEntry> {
    (0u32..0x00FF_FFFF, 0u8..=15, any::<bool>(), any::<bool>()).prop_map(
        |(handler_offset, handler_arity, side_effecting, control_flow)| OpcodeHandlerEntry {
            handler_offset,
            handler_arity,
            side_effecting,
            control_flow,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn pack_unpack_round_trips(entry in arb_entry()) {
        let packed = pack_dispatch_table(&[entry]).expect("valid entry");
        prop_assert_eq!(packed.len(), 1);
        prop_assert_eq!(unpack_entry(packed[0]), entry);
    }

    #[test]
    fn packed_table_len_matches_input(
        entries in proptest::collection::vec(arb_entry(), 0..=32),
    ) {
        let packed = pack_dispatch_table(&entries).expect("valid entries");
        prop_assert_eq!(packed.len(), entries.len());
        for (word, entry) in packed.iter().zip(entries.iter()) {
            prop_assert_eq!(unpack_entry(*word), *entry);
        }
    }

    #[test]
    fn arity_field_round_trips(arity in 0u8..=15) {
        let entry = OpcodeHandlerEntry {
            handler_offset: 42,
            handler_arity: arity,
            side_effecting: false,
            control_flow: false,
        };
        let packed = pack_dispatch_table(&[entry]).unwrap();
        prop_assert_eq!(unpack_entry(packed[0]).handler_arity, arity);
    }

    #[test]
    fn offset_field_round_trips(offset in 0u32..0x00FF_FFFF) {
        let entry = OpcodeHandlerEntry {
            handler_offset: offset,
            handler_arity: 0,
            side_effecting: false,
            control_flow: false,
        };
        let packed = pack_dispatch_table(&[entry]).unwrap();
        prop_assert_eq!(unpack_entry(packed[0]).handler_offset, offset);
    }
}
