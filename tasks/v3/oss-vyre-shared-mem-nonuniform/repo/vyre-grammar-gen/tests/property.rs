//! Property-based tests: proptest invariants over the public API.

use proptest::prelude::*;
use vyre_grammar_gen::{
    decode_dfa_from_bytes, decode_lr_from_bytes,
    kinds_blake3,
    wire::{PackedBlob, WireError},
    DfaBuilder, LrBuilder,
    lr::Action,
    validate_lr_table,
    dfa::{Transition, Action as DfaAction},
};

// ---------------------------------------------------------------------------
// Transition pack/unpack is a bijection
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn transition_pack_unpack_roundtrip(
        next_state in 0u16..=u16::MAX,
        action_tag in 0u32..4,
    ) {
        let action = match action_tag {
            0 => DfaAction::Continue,
            1 => DfaAction::EmitToken,
            2 => DfaAction::PushBack,
            _ => DfaAction::Error,
        };
        let t = Transition { next_state, action };
        let got = Transition::unpack(t.pack());
        prop_assert_eq!(got.next_state, next_state);
        prop_assert_eq!(got.action, action);
    }
}

// ---------------------------------------------------------------------------
// LR Action pack/unpack is a bijection for all representable payloads
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn lr_action_shift_roundtrip(state in 0u32..=0x0FFF_FFFFu32) {
        prop_assert_eq!(Action::unpack(Action::Shift(state).pack()), Action::Shift(state));
    }

    #[test]
    fn lr_action_reduce_roundtrip(prod in 0u32..=0x0FFF_FFFFu32) {
        prop_assert_eq!(Action::unpack(Action::Reduce(prod).pack()), Action::Reduce(prod));
    }
}

// ---------------------------------------------------------------------------
// DFA table wire round-trip: arbitrary table dimensions
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfa_wire_roundtrip_arbitrary_dims(
        states in 1u32..=16u32,
        classes in 1u32..=16u32,
    ) {
        let mut b = DfaBuilder::new(states, classes);
        // Wire a simple continue on (0,0) -> 0 so the table isn't trivially empty
        b.continue_to(0, 0, 0).expect("state 0 fits in u16");
        let dfa = b.build().expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        let got = decode_dfa_from_bytes(&blob.bytes).expect("decode must succeed");
        prop_assert_eq!(got.num_states, states);
        prop_assert_eq!(got.num_classes, classes);
        prop_assert_eq!(got.transitions.len(), (states * classes) as usize);
        prop_assert_eq!(got.token_ids.len(), states as usize);
    }
}

proptest! {
    #[test]
    fn dfa_wire_roundtrip_preserves_token_ids(
        states in 2u32..=8u32,
        classes in 2u32..=8u32,
        accept_state in 0u32..8u32,
        token_id in 1u32..=1000u32,
    ) {
        let accept_state = accept_state.min(states - 1);
        let mut b = DfaBuilder::new(states, classes);
        b.accept(accept_state, token_id);
        let dfa = b.build().expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        let got = decode_dfa_from_bytes(&blob.bytes).expect("decode");
        prop_assert_eq!(got.token_ids[accept_state as usize], token_id);
    }
}

// ---------------------------------------------------------------------------
// LR table wire round-trip
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn lr_wire_roundtrip_dimensions(
        states in 1u32..=8u32,
        tokens in 1u32..=8u32,
        nts in 1u32..=4u32,
    ) {
        let mut b = LrBuilder::new(states, tokens, nts);
        b.set_action(0, 0, Action::Accept);
        let lr = b.build();
        let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
        let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
        prop_assert_eq!(got.num_states, states);
        prop_assert_eq!(got.num_tokens, tokens);
        prop_assert_eq!(got.num_nonterminals, nts);
    }

    #[test]
    fn lr_wire_roundtrip_preserves_actions(
        states in 2u32..=6u32,
        tokens in 2u32..=6u32,
        nts in 1u32..=3u32,
        state in 0u32..6u32,
        tok in 0u32..6u32,
        target in 0u32..=0x0FFF_FFFFu32,
    ) {
        let state = state.min(states - 1);
        let tok = tok.min(tokens - 1);
        let mut b = LrBuilder::new(states, tokens, nts);
        b.set_action(state, tok, Action::Shift(target.min(states - 1)));
        let lr = b.build();
        let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
        let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
        prop_assert_eq!(
            got.action_at(state, tok),
            Action::Shift(target.min(states - 1))
        );
    }
}

// ---------------------------------------------------------------------------
// Wire blob: any truncation of a valid blob is rejected
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfa_truncated_blob_is_rejected(
        // Drop between 1 and 20 bytes from the end
        drop in 1usize..=20usize,
    ) {
        let dfa = DfaBuilder::new(4, 8).build().expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        let len = blob.bytes.len();
        if drop >= len {
            // Can't truncate more than the blob length; skip this sample
            return Ok(());
        }
        let truncated = &blob.bytes[..len - drop];
        let err = decode_dfa_from_bytes(truncated).unwrap_err();
        prop_assert!(
            matches!(
                err,
                WireError::TooShort { .. }
                    | WireError::PayloadTruncated { .. }
                    | WireError::ChecksumMismatch { .. }
                    | WireError::LexerPayloadWordCount { .. }
            ),
            "unexpected error: {err:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// validate_lr_table: builder-produced tables always pass validation
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn lr_builder_always_validates(
        states in 1u32..=10u32,
        tokens in 1u32..=10u32,
        nts in 1u32..=5u32,
    ) {
        let t = LrBuilder::new(states, tokens, nts).build();
        prop_assert!(validate_lr_table(&t).is_ok(),
            "builder-produced table failed validation for ({states},{tokens},{nts})");
    }
}

// ---------------------------------------------------------------------------
// kinds_blake3: deterministic for any kind sequence
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn kinds_blake3_deterministic(kinds in prop::collection::vec(0u32..=300u32, 0..=50)) {
        let h1 = kinds_blake3(&kinds);
        let h2 = kinds_blake3(&kinds);
        prop_assert_eq!(h1, h2);
    }

    #[test]
    fn kinds_blake3_different_sequences_have_different_hashes(
        a in prop::collection::vec(0u32..=300u32, 1..=20),
        b in prop::collection::vec(0u32..=300u32, 1..=20),
    ) {
        // Two distinct sequences should (with overwhelming probability) have different hashes
        prop_assume!(a != b);
        let ha = kinds_blake3(&a);
        let hb = kinds_blake3(&b);
        prop_assert_ne!(ha, hb);
    }
}

// ---------------------------------------------------------------------------
// DFA set_transition / transition lookup consistency
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfa_table_set_get_roundtrip(
        states in 2u32..=8u32,
        classes in 2u32..=8u32,
        state in 0u32..8u32,
        class in 0u32..8u32,
        next_state in 0u32..8u32,
    ) {
        let state = state.min(states - 1);
        let class = class.min(classes - 1);
        let next_state_val = (next_state.min(states - 1)) as u16;
        let b = DfaBuilder::new(states, classes);
        let action = DfaAction::Continue;
        let t_in = Transition { next_state: next_state_val, action };
        let mut table = b.build().expect("empty pattern set must succeed");
        table.set_transition(state, class, t_in);
        let t_out = table.transition(state, class);
        prop_assert_eq!(t_out.next_state, next_state_val);
        prop_assert_eq!(t_out.action, DfaAction::Continue);
    }
}
