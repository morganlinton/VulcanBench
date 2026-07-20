//! Adversarial emit program matrix for `vyre-emit-ptx`.
//!
//! Hostile `KernelDescriptor` programs from `vyre_lower::emit_adversarial_corpus`
//! with structural assertions on lowered PTX text.

use proptest::prelude::*;
use vyre_emit_ptx::EmitError;
use vyre_lower::emit_adversarial_corpus::{
    self, EmitAdversarialBackend, EmitAdversarialCase, EmitAdversarialFamily,
};

fn assert_ptx_structure(case: &EmitAdversarialCase, ptx: &str) {
    assert!(ptx.contains(".version"), "{}: missing .version", case.id);
    assert!(ptx.contains(".target"), "{}: missing .target", case.id);
    assert!(
        ptx.contains(".entry main"),
        "{}: missing .entry main",
        case.id
    );

    match case.family {
        EmitAdversarialFamily::DeepIfElse => {
            assert!(
                ptx.contains("$L_if_else_") || ptx.contains("$L_if_end_"),
                "{}: nested if/else must emit branch labels\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::HostileWorkgroup => {
            assert!(
                ptx.contains("ld.global") && ptx.contains("st.global"),
                "{}: workgroup-indexed store must touch global memory\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::MultiBinding => {
            assert!(
                ptx.matches(".param .u64").count() >= 3,
                "{}: ≥3 bindings must produce ≥3 u64 params\n{}",
                case.id,
                ptx
            );
            assert!(
                ptx.contains("st.global.f32") || ptx.contains("add.f32"),
                "{}: f32 binding must lower to f32 PTX ops\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::SharedGlobalTile => {
            assert!(
                ptx.contains("shared") || ptx.contains(".shared"),
                "{}: shared tile must allocate shared memory\n{}",
                case.id,
                ptx
            );
            assert!(
                ptx.contains("bar.sync"),
                "{}: tile kernel must emit workgroup barrier\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::LoopWithBarrier => {
            let has_structured_loop = ptx.contains("$L_for_head_") && ptx.contains("bar.sync");
            let has_precisely_unrolled_loop =
                ptx.matches("bar.sync").count() == 4 && ptx.matches("st.global.u32").count() == 4;
            assert!(
                has_structured_loop || has_precisely_unrolled_loop,
                "{}: loop+barrier must emit a structured loop or exact four-iteration unrolled CTA barrier/store sequence\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::VecLoadFusion => {
            assert!(
                ptx.contains("ld.global.nc.v4.u32") || ptx.contains("ld.global.v4.u32"),
                "{}: unit-stride quad load must fuse to vector load\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::AtomicCounter => {
            assert!(
                ptx.contains("atom.global"),
                "{}: atomic counter must emit global atom instruction\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::DeadIdentityChain => {
            assert!(
                ptx.contains("st.global.u32"),
                "{}: dead identity chain must still store the live literal\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::SignedBufferArithmetic => {
            assert!(
                ptx.contains("and.b32"),
                "{}: signed buffer bit-and must lower to a real and.b32\n{}",
                case.id,
                ptx
            );
            assert!(
                ptx.contains("shr.") && ptx.contains("st.global.u32"),
                "{}: signed buffer arithmetic must shift and store the narrowed result\n{}",
                case.id,
                ptx
            );
        }
        EmitAdversarialFamily::RejectCall | EmitAdversarialFamily::RejectGridSyncBarrier => {
            panic!(
                "{}: rejection case must not reach PTX structure oracle",
                case.id
            );
        }
    }
}

#[test]
fn hostile_success_corpus_emits_structured_ptx() {
    assert!(
        emit_adversarial_corpus::required_backends().contains(&EmitAdversarialBackend::Ptx),
        "Fix: shared emit adversarial corpus must register PTX as a required consumer."
    );

    for case in emit_adversarial_corpus::success_cases() {
        let ptx = vyre_emit_ptx::emit_optimized(&case.descriptor).unwrap_or_else(|err| {
            panic!(
                "Fix: `{}` ({:?}) must emit PTX: {err:?}",
                case.id, case.family
            )
        });
        assert_ptx_structure(&case, &ptx);
    }
}

#[test]
fn rejection_corpus_fails_without_panic() {
    for case in emit_adversarial_corpus::rejection_cases() {
        let result = vyre_emit_ptx::emit_optimized(&case.descriptor);
        assert!(
            result.is_err(),
            "Fix: `{}` must be rejected by PTX emit",
            case.id
        );
        if case.family == EmitAdversarialFamily::RejectGridSyncBarrier {
            match result {
                Err(EmitError::InvalidDescriptor(message)) => {
                    assert!(
                        message.contains("GridSync"),
                        "Fix: GridSync rejection must name scope loss; got: {message}"
                    );
                }
                other => panic!("Fix: GridSync must reject with InvalidDescriptor, got {other:?}"),
            }
        }
    }
}

#[test]
fn dead_identity_chain_optimized_ptx_is_not_longer_than_raw() {
    let case = emit_adversarial_corpus::case_by_id("adv_dead_identity").unwrap();
    let raw = vyre_emit_ptx::emit(&case.descriptor).expect("raw emit");
    let optimized = vyre_emit_ptx::emit_optimized(&case.descriptor).expect("optimized emit");
    assert!(
        optimized.lines().count() <= raw.lines().count(),
        "Fix: optimized PTX should not grow vs raw for dead identity chain"
    );
}

#[test]
fn vec_load_fusion_emits_no_scalar_load_fallback() {
    let case = emit_adversarial_corpus::case_by_id("adv_vec_load_fusion").unwrap();
    let ptx = vyre_emit_ptx::emit_optimized(&case.descriptor).unwrap();
    assert!(
        ptx.contains("ld.global.nc.v4.u32") || ptx.contains("ld.global.v4.u32"),
        "Fix: unit-stride quad load must fuse to a PTX vector load\n{ptx}"
    );
    assert_eq!(
        ptx.matches("ld.global.u32").count(),
        0,
        "Fix: fused vector load must not leave scalar ld.global.u32 behind\n{ptx}"
    );
    assert!(ptx.contains("st.global.u32"), "must store result\n{ptx}");
}

#[test]
fn shared_global_tile_touches_both_address_spaces() {
    let case = emit_adversarial_corpus::case_by_id("adv_shared_global_tile").unwrap();
    let ptx = vyre_emit_ptx::emit_optimized(&case.descriptor).unwrap();
    assert!(ptx.contains("ld.global"), "must load global\n{ptx}");
    assert!(ptx.contains("st.global"), "must store global\n{ptx}");
    assert!(
        ptx.contains("st.shared") || ptx.contains("ld.shared"),
        "must touch shared memory\n{ptx}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    #[test]
    fn success_corpus_ptx_contains_required_directives(case_index in 0usize..8) {
        let cases = emit_adversarial_corpus::success_cases();
        prop_assume!(case_index < cases.len());
        let case = &cases[case_index];
        let ptx = vyre_emit_ptx::emit_optimized(&case.descriptor)
            .expect("corpus success case must emit PTX");
        assert!(ptx.contains(".address_size"), "{}: missing .address_size", case.id);
        assert!(ptx.contains("ret;"), "{}: kernel must return", case.id);
    }
}
