//! Adversarial malformed-wire tests for the public `Program::from_wire` surface.
//!
//! This is the CI-level fuzz surrogate: arbitrary hostile bytes and every
//! truncation prefix of a valid program must either decode into a stable
//! canonical program or fail with an actionable error. Panics are always bugs.

use proptest::prelude::*;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn valid_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

fn assert_decode_result_is_stable(bytes: &[u8]) -> Result<(), TestCaseError> {
    let outcome = std::panic::catch_unwind(|| Program::from_wire(bytes));
    prop_assert!(
        outcome.is_ok(),
        "Program::from_wire panicked on {} hostile byte(s)",
        bytes.len()
    );
    match outcome.expect("checked above") {
        Ok(program) => {
            let encoded = program.to_wire().map_err(|error| {
                TestCaseError::fail(format!(
                    "decoded hostile wire bytes into a program that cannot re-encode: {error}"
                ))
            })?;
            let decoded_again = Program::from_wire(&encoded).map_err(|error| {
                TestCaseError::fail(format!(
                    "canonical re-encode from hostile bytes could not decode again: {error}"
                ))
            })?;
            prop_assert_eq!(
                decoded_again,
                program,
                "decoded hostile wire bytes must re-encode into a stable canonical program"
            );
        }
        Err(error) => {
            let message = error.to_string();
            prop_assert!(
                message.contains("Fix:"),
                "malformed-wire error must be actionable, got `{message}`"
            );
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 1024,
        ..ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_hostile_wire_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        assert_decode_result_is_stable(&bytes)?;
    }
}

#[test]
fn every_valid_wire_prefix_is_rejected_or_canonical_without_panic() {
    let bytes = valid_program()
        .to_wire()
        .expect("Fix: adversarial wire fixture must encode");
    assert!(
        bytes.len() > 16,
        "Fix: adversarial wire fixture must be large enough to exercise truncation prefixes"
    );
    for prefix_len in 0..bytes.len() {
        assert_decode_result_is_stable(&bytes[..prefix_len]).unwrap_or_else(|error| {
            panic!(
                "valid wire prefix length {prefix_len} violated malformed-wire contract: {error}"
            )
        });
    }
}

#[test]
fn single_byte_mutations_are_rejected_or_canonical_without_panic() {
    let bytes = valid_program()
        .to_wire()
        .expect("Fix: adversarial wire fixture must encode");
    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0xA5;
        assert_decode_result_is_stable(&mutated).unwrap_or_else(|error| {
            panic!(
                "single-byte mutation at offset {index} violated malformed-wire contract: {error}"
            )
        });
    }
}
