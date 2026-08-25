//! Resident C frontend megakernel workspace contracts.
//!
//! These tests verify the 0.6 no-host parser substrate: stable phase ids,
//! checked arena layout, capacity diagnostics, and builder ABI wiring.

use vyre_foundation::ir::{Expr, Node};
use vyre_frontend_c::megakernel_workspace::manifest_word;
use vyre_frontend_c::megakernel_workspace::{
    build_program_sharded_with_c_frontend_workspace,
    build_program_sharded_with_c_frontend_workspace_phases, c_frontend_advance_phase_nodes,
    CFrontendCapacityDiagnosticKind, CFrontendMegakernelWorkspace, CFrontendPhase,
    CFrontendPhaseHandler, CFrontendRegionId, CFrontendWorkspaceError, CFrontendWorkspaceLimits,
    C_FRONTEND_CONDITIONAL_WORDS, C_FRONTEND_DIAGNOSTIC_WORDS, C_FRONTEND_MACRO_WORDS,
    C_FRONTEND_MANIFEST_WORDS, C_FRONTEND_PG_EDGE_WORDS, C_FRONTEND_TOKEN_WORDS,
    C_FRONTEND_VAST_ROW_WORDS, C_FRONTEND_WORKSPACE_ABI_VERSION, C_FRONTEND_WORKSPACE_BINDING,
    C_FRONTEND_WORKSPACE_BUFFER, C_FRONTEND_WORKSPACE_MAGIC, C_FRONTEND_WORK_QUEUE_WORDS,
};
use vyre_runtime::megakernel::build_program_sharded_with_workspace_adapter;

#[test]
fn workspace_manifest_regions_are_ordered_and_non_overlapping() {
    let limits = CFrontendWorkspaceLimits::small_translation_unit();
    let manifest = limits.manifest().expect("small TU manifest must build");

    let mut previous_end = 0;
    for region in manifest.regions() {
        assert!(
            region.offset_words >= previous_end,
            "{:?} starts at {}, before prior end {previous_end}",
            region.id,
            region.offset_words
        );
        previous_end = region
            .end_words()
            .expect("region end must fit after manifest construction");
    }

    assert_eq!(manifest.manifest.offset_words, 0);
    assert_eq!(manifest.manifest.words, C_FRONTEND_MANIFEST_WORDS);
    assert_eq!(
        manifest.total_words(),
        manifest
            .work_queue
            .end_words()
            .expect("work queue end must fit")
    );
}

#[test]
fn workspace_manifest_records_capacity_words_exactly() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");

    assert_eq!(
        manifest.tokens.words,
        manifest.limits.token_capacity * C_FRONTEND_TOKEN_WORDS
    );
    assert_eq!(
        manifest.macros.words,
        manifest.limits.macro_capacity * C_FRONTEND_MACRO_WORDS
    );
    assert_eq!(
        manifest.conditionals.words,
        manifest.limits.conditional_capacity * C_FRONTEND_CONDITIONAL_WORDS
    );
    assert_eq!(
        manifest.vast_rows.words,
        manifest.limits.vast_row_capacity * C_FRONTEND_VAST_ROW_WORDS
    );
    assert_eq!(
        manifest.pg_edges.words,
        manifest.limits.pg_edge_capacity * C_FRONTEND_PG_EDGE_WORDS
    );
    assert_eq!(
        manifest.diagnostics.words,
        manifest.limits.diagnostic_capacity * C_FRONTEND_DIAGNOSTIC_WORDS
    );
    assert_eq!(
        manifest.work_queue.words,
        manifest.limits.work_queue_capacity * C_FRONTEND_WORK_QUEUE_WORDS
    );
}

#[test]
fn workspace_manifest_rejects_zero_capacity_regions() {
    let mut limits = CFrontendWorkspaceLimits::small_translation_unit();
    limits.token_capacity = 0;
    let err = limits
        .manifest()
        .expect_err("zero token capacity must reject");

    assert_eq!(
        err,
        CFrontendWorkspaceError::ZeroCapacity {
            region: CFrontendRegionId::Tokens,
        }
    );
    assert!(
        err.to_string().contains("Fix:"),
        "capacity error must be actionable: {err}"
    );
}

#[test]
fn workspace_manifest_rejects_total_word_cap_before_builder_use() {
    let limits = CFrontendWorkspaceLimits {
        source_bytes: u32::MAX,
        token_capacity: 1,
        macro_capacity: 1,
        conditional_capacity: 1,
        vast_row_capacity: 1,
        pg_edge_capacity: 1,
        diagnostic_capacity: 1,
        work_queue_capacity: 1,
    };
    let err = limits
        .manifest()
        .expect_err("oversized source region must reject");

    assert!(
        matches!(err, CFrontendWorkspaceError::WorkspaceTooLarge { .. }),
        "expected workspace cap rejection, got {err:?}"
    );
    assert!(
        err.to_string().contains("Fix:"),
        "workspace cap error must be actionable: {err}"
    );
}

#[test]
fn phase_ids_are_stable_and_linear() {
    assert_eq!(CFrontendPhase::ResidentReady.id(), 0);
    assert_eq!(CFrontendPhase::Complete.id(), 11);
    assert_eq!(CFrontendPhase::Fault.id(), 12);
    assert_eq!(
        CFrontendPhase::from_id(CFrontendPhase::PgLower.id()),
        Some(CFrontendPhase::PgLower)
    );

    let ordered = [
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

    for pair in ordered.windows(2) {
        vyre_frontend_c::megakernel_workspace::validate_c_frontend_phase_transition(
            pair[0], pair[1],
        )
        .expect("adjacent parser phases must transition");
    }
}

#[test]
fn phase_transition_rejects_success_path_skips() {
    let err = vyre_frontend_c::megakernel_workspace::validate_c_frontend_phase_transition(
        CFrontendPhase::Lex,
        CFrontendPhase::VastBuild,
    )
    .expect_err("parser phase machine must reject skipped phases");

    assert!(
        matches!(
            err,
            CFrontendWorkspaceError::InvalidPhaseTransition {
                from: CFrontendPhase::Lex,
                to: CFrontendPhase::VastBuild
            }
        ),
        "unexpected phase error: {err:?}"
    );
    assert!(
        err.to_string().contains("Fix:"),
        "phase error must be actionable: {err}"
    );
}

#[test]
fn builder_wires_c_frontend_workspace_buffer_after_existing_megakernel_buffers() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");
    let program = build_program_sharded_with_c_frontend_workspace(64, 128, &[], &manifest);

    let buffer = program
        .buffer(C_FRONTEND_WORKSPACE_BUFFER)
        .expect("C frontend workspace buffer must be declared");
    assert_eq!(buffer.binding, C_FRONTEND_WORKSPACE_BINDING);
    assert_eq!(buffer.count, manifest.total_words());
    assert!(buffer.pipeline_live_out);

    assert!(
        program.buffer("control").is_some(),
        "workspace builder must preserve legacy control buffer"
    );
    assert!(
        program.buffer("ring_buffer").is_some(),
        "workspace builder must preserve legacy ring buffer"
    );
}

#[test]
fn generic_workspace_adapter_builds_same_resident_buffer_seam() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");
    let adapter = CFrontendMegakernelWorkspace::new(&manifest, &[]);
    let program = build_program_sharded_with_workspace_adapter(64, 128, &[], &adapter);

    let buffer = program
        .buffer(C_FRONTEND_WORKSPACE_BUFFER)
        .expect("generic workspace adapter must declare the resident buffer");
    assert_eq!(buffer.binding, C_FRONTEND_WORKSPACE_BINDING);
    assert_eq!(buffer.count, manifest.total_words());
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::MAGIC,
            C_FRONTEND_WORKSPACE_MAGIC,
        ),
        "generic adapter path must preserve domain-owned GPU bootstrap IR"
    );
}

#[test]
fn manifest_header_word_table_fits_manifest_region() {
    let highest_fixed_word = manifest_word::REGION_TABLE_BASE
        + (CFrontendRegionId::WorkQueue.id() + 1) * manifest_word::REGION_TABLE_ENTRY_WORDS;
    assert!(
        highest_fixed_word <= C_FRONTEND_MANIFEST_WORDS,
        "manifest region table must fit fixed manifest words"
    );
    assert_eq!(C_FRONTEND_WORKSPACE_MAGIC, 0x5659_4346);
    assert_eq!(C_FRONTEND_WORKSPACE_ABI_VERSION, 1);
}

#[test]
fn workspace_builder_bootstraps_manifest_on_gpu_without_faking_completion() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");
    let program = build_program_sharded_with_c_frontend_workspace(64, 128, &[], &manifest);

    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::MAGIC,
            C_FRONTEND_WORKSPACE_MAGIC,
        ),
        "resident megakernel must write workspace magic on GPU"
    );
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::CURRENT_PHASE,
            CFrontendPhase::ResidentReady.id(),
        ),
        "bootstrap must leave parser at resident-ready until phase handlers are wired"
    );
    assert!(
        !stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::CURRENT_PHASE,
            CFrontendPhase::Complete.id(),
        ),
        "empty phase set must not claim the parser completed"
    );
}

#[test]
fn workspace_phase_handlers_are_gpu_ir_and_advance_linearly() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");
    let mut ingest_body = vec![Node::store(
        C_FRONTEND_WORKSPACE_BUFFER,
        Expr::u32(manifest_word::SOURCE_BYTES),
        Expr::u32(128),
    )];
    ingest_body.extend(
        c_frontend_advance_phase_nodes(CFrontendPhase::ResidentReady, CFrontendPhase::Ingest)
            .expect("adjacent phase transition must build"),
    );

    let handler = CFrontendPhaseHandler::new(CFrontendPhase::ResidentReady, ingest_body);
    let program =
        build_program_sharded_with_c_frontend_workspace_phases(64, 128, &[], &manifest, &[handler]);

    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::SOURCE_BYTES,
            128,
        ),
        "phase body must be present as resident GPU IR"
    );
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::REQUESTED_PHASE,
            CFrontendPhase::Ingest.id(),
        ),
        "phase body must request the next resident phase"
    );
}

#[test]
fn workspace_phase_guard_records_invalid_transition_diagnostic_words() {
    let manifest = CFrontendWorkspaceLimits::small_translation_unit()
        .manifest()
        .expect("small TU manifest must build");
    let program = build_program_sharded_with_c_frontend_workspace(64, 128, &[], &manifest);

    assert!(
        loads_word(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::CURRENT_PHASE,
        ),
        "phase guard must read the resident current phase"
    );
    assert!(
        loads_word(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::REQUESTED_PHASE,
        ),
        "phase guard must read the resident requested phase"
    );
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::DIAGNOSTIC_KIND,
            CFrontendCapacityDiagnosticKind::PhaseTransition.id(),
        ),
        "invalid resident phase requests must publish a diagnostic kind"
    );
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::STATUS,
            CFrontendCapacityDiagnosticKind::PhaseTransition.id(),
        ),
        "invalid resident phase requests must set a non-zero status word"
    );
    assert!(
        stores_literal(
            &program.entry,
            C_FRONTEND_WORKSPACE_BUFFER,
            manifest_word::DIAGNOSTIC_COUNT,
            1,
        ),
        "invalid resident phase requests must record a diagnostic count"
    );
}

fn stores_literal(nodes: &[Node], buffer_name: &str, index: u32, value: u32) -> bool {
    nodes.iter().any(|node| match node {
        Node::Store {
            buffer,
            index: store_index,
            value: store_value,
        } => {
            buffer.as_str() == buffer_name
                && matches!(store_index, Expr::LitU32(actual) if *actual == index)
                && matches!(store_value, Expr::LitU32(actual) if *actual == value)
        }
        Node::If {
            then, otherwise, ..
        } => {
            stores_literal(then, buffer_name, index, value)
                || stores_literal(otherwise, buffer_name, index, value)
        }
        Node::Loop { body, .. } | Node::Block(body) => {
            stores_literal(body, buffer_name, index, value)
        }
        Node::Region { body, .. } => stores_literal(body, buffer_name, index, value),
        _ => false,
    })
}

fn loads_word(nodes: &[Node], buffer_name: &str, index: u32) -> bool {
    fn expr_loads_word(expr: &Expr, buffer_name: &str, index: u32) -> bool {
        match expr {
            Expr::Load {
                buffer,
                index: load_index,
            } => {
                buffer.as_str() == buffer_name
                    && matches!(load_index.as_ref(), Expr::LitU32(actual) if *actual == index)
            }
            Expr::BinOp { left, right, .. } => {
                expr_loads_word(left, buffer_name, index)
                    || expr_loads_word(right, buffer_name, index)
            }
            Expr::Atomic {
                index: atomic_index,
                expected,
                value,
                ..
            } => {
                expr_loads_word(atomic_index, buffer_name, index)
                    || expected
                        .as_ref()
                        .is_some_and(|expr| expr_loads_word(expr, buffer_name, index))
                    || expr_loads_word(value, buffer_name, index)
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                expr_loads_word(cond, buffer_name, index)
                    || expr_loads_word(true_val, buffer_name, index)
                    || expr_loads_word(false_val, buffer_name, index)
            }
            _ => false,
        }
    }

    nodes.iter().any(|node| match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            expr_loads_word(value, buffer_name, index)
        }
        Node::Store {
            index: store_index,
            value,
            ..
        } => {
            expr_loads_word(store_index, buffer_name, index)
                || expr_loads_word(value, buffer_name, index)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_loads_word(cond, buffer_name, index)
                || loads_word(then, buffer_name, index)
                || loads_word(otherwise, buffer_name, index)
        }
        Node::Loop { from, to, body, .. } => {
            expr_loads_word(from, buffer_name, index)
                || expr_loads_word(to, buffer_name, index)
                || loads_word(body, buffer_name, index)
        }
        Node::Block(body) => loads_word(body, buffer_name, index),
        Node::Region { body, .. } => loads_word(body, buffer_name, index),
        _ => false,
    })
}
