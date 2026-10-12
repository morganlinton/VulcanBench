//! Adversarial emit program matrix for `vyre-emit-metal`.
//!
//! Hostile `KernelDescriptor` programs from `vyre_lower::emit_adversarial_corpus`
//! must either emit a structured Metal native_module artifact or return a typed
//! emitter error with fix text.

use vyre_emit_metal::{EmitError, METAL_ARTIFACT_SCHEMA};
use vyre_lower::emit_adversarial_corpus::{
    self, EmitAdversarialBackend, EmitAdversarialCase, EmitAdversarialFamily,
};

fn assert_metal_artifact_structure(
    case: &EmitAdversarialCase,
    artifact: &vyre_emit_metal::MetalArtifact,
) {
    assert_eq!(
        artifact.schema, METAL_ARTIFACT_SCHEMA,
        "{}: schema must match crate constant",
        case.id
    );
    assert!(
        artifact.entry_point == "main" || artifact.entry_point == "main_",
        "{}: Metal artifact must expose compute entry derived from `main`, got `{}`",
        case.id,
        artifact.entry_point
    );
    assert!(
        artifact.msl.contains(&artifact.entry_point),
        "{}: Metal source must contain artifact entry point `{}`\n{}",
        case.id,
        artifact.entry_point,
        artifact.msl
    );
    assert_eq!(
        artifact.workgroup_size, case.descriptor.dispatch.workgroup_size,
        "{}: workgroup size must round-trip into Metal metadata",
        case.id
    );
    assert!(
        artifact.descriptor_blake3.len() == 64 && artifact.msl_blake3.len() == 64,
        "{}: artifact digests must be hex blake3 values",
        case.id
    );
    assert!(
        artifact.msl.contains("kernel"),
        "{}: artifact must contain Metal kernel source\n{}",
        case.id,
        artifact.msl
    );

    match case.family {
        EmitAdversarialFamily::MultiBinding => {
            assert!(
                artifact.bindings.len() >= 3,
                "{}: multi-binding descriptor must preserve binding metadata",
                case.id
            );
        }
        EmitAdversarialFamily::SharedGlobalTile => {
            assert!(
                !artifact.threadgroup_memories.is_empty() || artifact.msl.contains("threadgroup"),
                "{}: shared tile must expose threadgroup memory metadata or source",
                case.id
            );
        }
        EmitAdversarialFamily::HostileWorkgroup => {
            assert_eq!(
                artifact.workgroup_size,
                [1024, 1, 1],
                "{}: hostile workgroup size must survive Metal artifact creation",
                case.id
            );
        }
        EmitAdversarialFamily::DeepIfElse
        | EmitAdversarialFamily::LoopWithBarrier
        | EmitAdversarialFamily::AtomicCounter
        | EmitAdversarialFamily::DeadIdentityChain
        | EmitAdversarialFamily::VecLoadFusion
        | EmitAdversarialFamily::SignedBufferArithmetic => {}
        EmitAdversarialFamily::RejectCall | EmitAdversarialFamily::RejectGridSyncBarrier => {
            panic!(
                "{}: rejection case must not reach Metal artifact structure oracle",
                case.id
            );
        }
    }
}

fn assert_structured_metal_error(case: &EmitAdversarialCase, error: EmitError) {
    match error {
        EmitError::NagaEmit(message)
        | EmitError::NagaValidation(message)
        | EmitError::MslWriter(message)
        | EmitError::DescriptorHash(message)
        | EmitError::ArtifactSerialization(message)
        | EmitError::PreEmit(message) => {
            assert!(
                !message.is_empty(),
                "Fix: `{}` Metal rejection must carry diagnostic text.",
                case.id
            );
        }
        EmitError::EntryPoint {
            entry_point,
            reason,
        } => {
            assert!(
                !entry_point.is_empty() && !reason.is_empty(),
                "Fix: `{}` Metal entry-point rejection must name entry and reason.",
                case.id
            );
        }
        EmitError::BindingMap {
            group,
            binding,
            reason,
        } => {
            assert!(
                !reason.is_empty() || group > 0 || binding > 0,
                "Fix: `{}` Metal binding-map rejection must identify the failed binding.",
                case.id
            );
        }
    }
}

#[test]
fn hostile_success_corpus_emits_structured_metal_artifacts() {
    assert!(
        emit_adversarial_corpus::required_backends().contains(&EmitAdversarialBackend::Metal),
        "Fix: shared emit adversarial corpus must register Metal as a required consumer."
    );

    for case in emit_adversarial_corpus::success_cases() {
        let artifact = vyre_emit_metal::emit_artifact(&case.descriptor).unwrap_or_else(|err| {
            panic!(
                "Fix: `{}` ({:?}) must emit Metal native_module artifact: {err:?}",
                case.id, case.family
            )
        });
        assert_metal_artifact_structure(&case, &artifact);
    }
}

#[test]
fn rejection_corpus_returns_structured_metal_errors() {
    for case in emit_adversarial_corpus::rejection_cases() {
        let error = vyre_emit_metal::emit_artifact(&case.descriptor)
            .expect_err("Fix: rejection corpus case must be rejected by Metal emit");
        assert_structured_metal_error(&case, error);
    }
}
