//! Adversarial emit program matrix for `vyre-emit-spirv`.
//!
//! Hostile `KernelDescriptor` programs from `vyre_lower::emit_adversarial_corpus`
//! with structural assertions on lowered SPIR-V words.

use proptest::prelude::*;
use vyre_emit_spirv::{EmitError, SPIRV_MAGIC};
use vyre_lower::emit_adversarial_corpus::{
    self, EmitAdversarialBackend, EmitAdversarialCase, EmitAdversarialFamily,
};

/// SPIR-V OpEntryPoint = 15, OpMemoryBarrier = 39, OpLoopMerge = 246.
const OP_ENTRY_POINT: u32 = 15;
const OP_MEMORY_BARRIER: u32 = 39;
const OP_LOOP_MERGE: u32 = 246;

fn spirv_contains_op(words: &[u32], op: u32) -> bool {
    words.iter().any(|word| word & 0xffff == op)
}

fn assert_spirv_structure(case: &EmitAdversarialCase, words: &[u32]) {
    assert_eq!(words[0], SPIRV_MAGIC, "{}: missing SPIR-V magic", case.id);
    assert!(
        words.len() > 20,
        "{}: real kernel must emit more than header-only SPIR-V",
        case.id
    );
    assert!(
        spirv_contains_op(words, OP_ENTRY_POINT),
        "{}: must contain OpEntryPoint",
        case.id
    );

    match case.family {
        EmitAdversarialFamily::DeepIfElse => {
            assert!(
                spirv_contains_op(words, OP_LOOP_MERGE) || words.len() > 48,
                "{}: nested if/else must emit structured CFG ops",
                case.id
            );
        }
        EmitAdversarialFamily::LoopWithBarrier => {
            assert!(
                spirv_contains_op(words, OP_MEMORY_BARRIER),
                "{}: loop+barrier must emit OpMemoryBarrier",
                case.id
            );
        }
        EmitAdversarialFamily::SharedGlobalTile => {
            assert!(
                spirv_contains_op(words, OP_MEMORY_BARRIER),
                "{}: shared tile must emit OpMemoryBarrier",
                case.id
            );
        }
        EmitAdversarialFamily::HostileWorkgroup => {
            assert_eq!(
                case.descriptor.dispatch.workgroup_size,
                [1024, 1, 1],
                "{}: corpus dispatch must stay hostile",
                case.id
            );
        }
        EmitAdversarialFamily::MultiBinding => {
            assert!(
                words.len() > 32,
                "{}: multi-binding kernel must produce non-trivial SPIR-V",
                case.id
            );
        }
        EmitAdversarialFamily::AtomicCounter => {
            assert!(
                words.len() > 40,
                "{}: atomic kernel must produce substantial SPIR-V",
                case.id
            );
        }
        EmitAdversarialFamily::DeadIdentityChain
        | EmitAdversarialFamily::VecLoadFusion
        | EmitAdversarialFamily::SignedBufferArithmetic => {}
        EmitAdversarialFamily::RejectCall | EmitAdversarialFamily::RejectGridSyncBarrier => {
            panic!(
                "{}: rejection case must not reach SPIR-V structure oracle",
                case.id
            );
        }
    }
}

#[test]
fn hostile_success_corpus_emits_structured_spirv() {
    assert!(
        emit_adversarial_corpus::required_backends()
            .contains(&EmitAdversarialBackend::Spirv),
        "Fix: shared emit adversarial corpus must register SPIR-V as a required consumer."
    );

    for case in emit_adversarial_corpus::success_cases() {
        let words = vyre_emit_spirv::emit_optimized(&case.descriptor).unwrap_or_else(|err| {
            panic!(
                "Fix: `{}` ({:?}) must emit SPIR-V: {err:?}",
                case.id, case.family
            )
        });
        assert_spirv_structure(&case, &words);
    }
}

#[test]
fn rejection_corpus_fails_without_panic() {
    for case in emit_adversarial_corpus::rejection_cases() {
        let result = vyre_emit_spirv::emit_optimized(&case.descriptor);
        assert!(
            result.is_err(),
            "Fix: `{}` must be rejected by SPIR-V emit",
            case.id
        );
        assert!(
            matches!(
                result,
                Err(EmitError::NagaEmit(_)) | Err(EmitError::NagaValidation(_))
            ),
            "Fix: `{}` rejection must surface naga-layer error, got {:?}",
            case.id,
            result.err()
        );
    }
}

#[test]
fn dead_identity_chain_optimized_spirv_is_not_longer_than_raw() {
    let case = emit_adversarial_corpus::case_by_id("adv_dead_identity").unwrap();
    let raw = vyre_emit_spirv::emit(&case.descriptor).expect("raw emit");
    let optimized = vyre_emit_spirv::emit_optimized(&case.descriptor).expect("optimized emit");
    assert!(
        optimized.len() <= raw.len(),
        "Fix: optimized SPIR-V ({} words) must not exceed raw ({} words)",
        optimized.len(),
        raw.len()
    );
}

#[test]
fn spirv_bytes_match_words_endianness_on_corpus() {
    for case in emit_adversarial_corpus::success_cases() {
        let words = vyre_emit_spirv::emit_optimized(&case.descriptor).unwrap();
        let bytes = vyre_emit_spirv::emit_optimized_bytes(&case.descriptor).unwrap();
        assert_eq!(bytes.len(), words.len() * 4, "{}", case.id);
        let first = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(first, SPIRV_MAGIC, "{}", case.id);
    }
}

#[test]
fn naga_module_path_matches_direct_emit_on_corpus() {
    for case in emit_adversarial_corpus::success_cases() {
        let via_naga = vyre_emit_naga::emit_optimized(&case.descriptor).unwrap();
        let direct = vyre_emit_spirv::emit_from_naga_module(&via_naga).unwrap();
        let pipeline = vyre_emit_spirv::emit_optimized(&case.descriptor).unwrap();
        assert_eq!(
            direct.len(),
            pipeline.len(),
            "{}: emit_from_naga_module and emit_optimized must agree on word count",
            case.id
        );
        assert_eq!(direct[0], pipeline[0], "{}", case.id);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    #[test]
    fn success_corpus_spirv_magic_and_size_invariant(case_index in 0usize..8) {
        let cases = emit_adversarial_corpus::success_cases();
        prop_assume!(case_index < cases.len());
        let case = &cases[case_index];
        let words = vyre_emit_spirv::emit_optimized(&case.descriptor)
            .expect("corpus success case must emit SPIR-V");
        assert_eq!(words[0], SPIRV_MAGIC);
        prop_assert!(words.len() > 16);
    }
}
