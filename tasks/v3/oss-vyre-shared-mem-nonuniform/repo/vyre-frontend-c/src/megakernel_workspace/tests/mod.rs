use super::*;
use vyre_foundation::ir::{Expr, Node};

fn small_limits() -> CFrontendWorkspaceLimits {
    CFrontendWorkspaceLimits::small_translation_unit()
}

// ── Manifest construction ──────────────────────────────────────────

#[test]
fn small_translation_unit_manifest_builds_successfully() {
    let manifest = small_limits().manifest().expect("Fix: small TU must build");
    assert!(manifest.total_words() > 0);
    assert!(manifest.total_words() <= MAX_C_FRONTEND_WORKSPACE_WORDS);
}

#[test]
fn manifest_regions_cover_all_nine() {
    let manifest = small_limits().manifest().unwrap();
    let regions = manifest.regions();
    assert_eq!(regions.len(), 9);
    // All region ids are distinct.
    let ids: Vec<u32> = regions.iter().map(|r| r.id.id()).collect();
    let unique: std::collections::HashSet<u32> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "region ids must be unique");
}

#[test]
fn manifest_regions_are_contiguous_and_non_overlapping() {
    let manifest = small_limits().manifest().unwrap();
    let regions = manifest.regions();
    for window in regions.windows(2) {
        let (prev, next) = (&window[0], &window[1]);
        let prev_end = prev.end_words().unwrap();
        assert_eq!(
            prev_end, next.offset_words,
            "region {:?} end ({prev_end}) != region {:?} start ({})",
            prev.id, next.id, next.offset_words
        );
    }
}

#[test]
fn manifest_total_words_equals_last_region_end() {
    let manifest = small_limits().manifest().unwrap();
    let regions = manifest.regions();
    let last = regions.last().unwrap();
    assert_eq!(
        manifest.total_words(),
        last.end_words().unwrap(),
        "total_words must equal the end of the last region"
    );
}

#[test]
fn manifest_buffer_decl_uses_correct_binding() {
    let manifest = small_limits().manifest().unwrap();
    let decl = manifest.buffer_decl();
    assert_eq!(decl.name(), C_FRONTEND_WORKSPACE_BUFFER);
}

// ── Capacity validation ────────────────────────────────────────────

#[test]
fn zero_source_bytes_rejected() {
    let mut limits = small_limits();
    limits.source_bytes = 0;
    match limits.manifest() {
        Err(CFrontendWorkspaceError::ZeroCapacity { region }) => {
            assert_eq!(region, CFrontendRegionId::SourceBytes);
        }
        other => panic!("expected ZeroCapacity for SourceBytes, got {other:?}"),
    }
}

#[test]
fn zero_token_capacity_rejected() {
    let mut limits = small_limits();
    limits.token_capacity = 0;
    match limits.manifest() {
        Err(CFrontendWorkspaceError::ZeroCapacity { region }) => {
            assert_eq!(region, CFrontendRegionId::Tokens);
        }
        other => panic!("expected ZeroCapacity for Tokens, got {other:?}"),
    }
}

#[test]
fn zero_work_queue_capacity_rejected() {
    let mut limits = small_limits();
    limits.work_queue_capacity = 0;
    match limits.manifest() {
        Err(CFrontendWorkspaceError::ZeroCapacity { region }) => {
            assert_eq!(region, CFrontendRegionId::WorkQueue);
        }
        other => panic!("expected ZeroCapacity for WorkQueue, got {other:?}"),
    }
}

#[test]
fn overflow_token_arena_rejected() {
    let mut limits = small_limits();
    limits.token_capacity = u32::MAX;
    match limits.manifest() {
        Err(CFrontendWorkspaceError::WordOverflow { region, .. }) => {
            assert_eq!(region, CFrontendRegionId::Tokens);
        }
        other => panic!("expected WordOverflow for Tokens, got {other:?}"),
    }
}

#[test]
fn workspace_too_large_rejected() {
    let mut limits = small_limits();
    // 8M VAST rows × 8 words = 64M words = MAX_C_FRONTEND_WORKSPACE_WORDS.
    // But with other regions, it exceeds the cap.
    limits.vast_row_capacity = 8 * 1024 * 1024;
    match limits.manifest() {
        Err(CFrontendWorkspaceError::WorkspaceTooLarge { .. }) => {}
        other => panic!("expected WorkspaceTooLarge, got {other:?}"),
    }
}

// ── Phase machine ──────────────────────────────────────────────────

#[test]
fn valid_sequential_transitions() {
    let phases = [
        CFrontendPhase::ResidentReady,
        CFrontendPhase::Ingest,
        CFrontendPhase::Lex,
        CFrontendPhase::DirectiveClassify,
        CFrontendPhase::MacroExpand,
        CFrontendPhase::ConditionalMask,
        CFrontendPhase::KeywordPromote,
        CFrontendPhase::VastBuild,
        CFrontendPhase::SemanticClassify,
        CFrontendPhase::PgLower,
        CFrontendPhase::Validate,
        CFrontendPhase::Complete,
    ];
    for window in phases.windows(2) {
        assert!(
            is_valid_c_frontend_phase_transition(window[0], window[1]),
            "{:?} -> {:?} must be valid",
            window[0],
            window[1]
        );
    }
}

#[test]
fn complete_to_resident_ready_is_valid() {
    assert!(is_valid_c_frontend_phase_transition(
        CFrontendPhase::Complete,
        CFrontendPhase::ResidentReady
    ));
}

#[test]
fn any_phase_to_fault_is_valid() {
    let all = [
        CFrontendPhase::ResidentReady,
        CFrontendPhase::Ingest,
        CFrontendPhase::Lex,
        CFrontendPhase::Validate,
        CFrontendPhase::Complete,
    ];
    for phase in all {
        assert!(
            is_valid_c_frontend_phase_transition(phase, CFrontendPhase::Fault),
            "{phase:?} -> Fault must be valid"
        );
    }
}

#[test]
fn fault_to_anything_is_invalid() {
    let targets = [
        CFrontendPhase::ResidentReady,
        CFrontendPhase::Ingest,
        CFrontendPhase::Complete,
    ];
    for target in targets {
        assert!(
            !is_valid_c_frontend_phase_transition(CFrontendPhase::Fault, target),
            "Fault -> {target:?} must be invalid"
        );
    }
}

#[test]
fn skipping_a_phase_is_invalid() {
    assert!(!is_valid_c_frontend_phase_transition(
        CFrontendPhase::ResidentReady,
        CFrontendPhase::Lex // skips Ingest
    ));
    assert!(!is_valid_c_frontend_phase_transition(
        CFrontendPhase::Lex,
        CFrontendPhase::VastBuild // skips DirectiveClassify + MacroExpand + ConditionalMask + KeywordPromote
    ));
}

#[test]
fn validate_c_frontend_phase_transition_returns_error_on_illegal() {
    let result =
        validate_c_frontend_phase_transition(CFrontendPhase::Fault, CFrontendPhase::Complete);
    match result {
        Err(CFrontendWorkspaceError::InvalidPhaseTransition { from, to }) => {
            assert_eq!(from, CFrontendPhase::Fault);
            assert_eq!(to, CFrontendPhase::Complete);
        }
        other => panic!("expected InvalidPhaseTransition, got {other:?}"),
    }
}

// ── IR generation ──────────────────────────────────────────────────

#[test]
fn bootstrap_nodes_are_nonempty() {
    let manifest = small_limits().manifest().unwrap();
    let nodes = c_frontend_workspace_bootstrap_nodes(&manifest);
    assert_eq!(nodes.len(), 1, "bootstrap must emit one gid-gated IR root");
}

#[test]
fn phase_dispatch_nodes_empty_handlers() {
    let nodes = c_frontend_phase_dispatch_nodes(&[]);
    assert_eq!(
        nodes.len(),
        1,
        "dispatch must emit one gid-gated control fragment even with no handlers"
    );
}

#[test]
fn phase_dispatch_nodes_with_handler() {
    let handlers = vec![CFrontendPhaseHandler::new(
        CFrontendPhase::Lex,
        vec![Node::let_bind("lex_done", Expr::u32(1))],
    )];
    let nodes = c_frontend_phase_dispatch_nodes(&handlers);
    let debug = format!("{nodes:?}");
    assert!(debug.contains("lex_done"), "handler body must be embedded");
}

#[test]
fn phase_machine_guard_nodes_are_nonempty() {
    let nodes = c_frontend_phase_machine_guard_nodes();
    assert_eq!(nodes.len(), 1, "guard must emit one gid-gated IR root");
}

#[test]
fn advance_phase_valid_produces_ir() {
    let nodes =
        c_frontend_advance_phase_nodes(CFrontendPhase::Lex, CFrontendPhase::DirectiveClassify)
            .unwrap();
    assert!(nodes.len() >= 2, "advance must produce store + CAS + guard");
}

#[test]
fn advance_phase_invalid_rejected() {
    let result = c_frontend_advance_phase_nodes(
        CFrontendPhase::Lex,
        CFrontendPhase::Complete, // illegal skip
    );
    match result {
        Err(CFrontendWorkspaceError::InvalidPhaseTransition { from, to }) => {
            assert_eq!(from, CFrontendPhase::Lex);
            assert_eq!(to, CFrontendPhase::Complete);
        }
        other => panic!("expected InvalidPhaseTransition, got {other:?}"),
    }
}

#[test]
fn fault_nodes_produce_seven_stores() {
    let nodes = c_frontend_fault_nodes(
        CFrontendCapacityDiagnosticKind::Tokens,
        CFrontendRegionId::Tokens,
        100,
        50,
    );
    assert_eq!(
        nodes.len(),
        7,
        "fault must write: kind, region, required, capacity, count, status, phase"
    );
}

// ── Constants / enums ──────────────────────────────────────────────

#[test]
fn region_ids_are_sequential() {
    let ids = [
        CFrontendRegionId::Manifest,
        CFrontendRegionId::SourceBytes,
        CFrontendRegionId::Tokens,
        CFrontendRegionId::Macros,
        CFrontendRegionId::Conditionals,
        CFrontendRegionId::VastRows,
        CFrontendRegionId::PgEdges,
        CFrontendRegionId::Diagnostics,
        CFrontendRegionId::WorkQueue,
    ];
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.id(), i as u32, "region ids must be 0..8 sequential");
    }
}

#[test]
fn phase_ids_are_sequential() {
    let phases = [
        CFrontendPhase::ResidentReady,
        CFrontendPhase::Ingest,
        CFrontendPhase::Lex,
        CFrontendPhase::DirectiveClassify,
        CFrontendPhase::MacroExpand,
        CFrontendPhase::ConditionalMask,
        CFrontendPhase::KeywordPromote,
        CFrontendPhase::VastBuild,
        CFrontendPhase::SemanticClassify,
        CFrontendPhase::PgLower,
        CFrontendPhase::Validate,
        CFrontendPhase::Complete,
        CFrontendPhase::Fault,
    ];
    for (i, phase) in phases.iter().enumerate() {
        assert_eq!(phase.id(), i as u32, "phase ids must be 0..12 sequential");
    }
}

#[test]
fn workspace_magic_is_nonzero() {
    assert_ne!(C_FRONTEND_WORKSPACE_MAGIC, 0);
}
