//! Builders that emit the `Vec<Node>` graphs for the C frontend
//! megakernel control flow (bootstrap, dispatch, guard, advance, fault).

use vyre_foundation::ir::{Expr, Node};

use super::error::{validate_c_frontend_phase_transition, CFrontendWorkspaceError};
use super::{
    manifest_word, CFrontendCapacityDiagnosticKind, CFrontendPhase, CFrontendPhaseHandler,
    CFrontendRegionId, CFrontendWorkspaceManifest, CFrontendWorkspaceRegion,
    C_FRONTEND_WORKSPACE_ABI_VERSION, C_FRONTEND_WORKSPACE_BUFFER, C_FRONTEND_WORKSPACE_MAGIC,
};

/// GPU IR that initializes the resident workspace manifest in-place.
///
/// Launcher-safe: the CPU supplies only compile-time ABI constants in the
/// program body; the megakernel writes magic/version/region layout on device
/// when the workspace is uninitialized.
#[must_use]
pub fn c_frontend_workspace_bootstrap_nodes(manifest: &CFrontendWorkspaceManifest) -> Vec<Node> {
    let mut init = vec![
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::MAGIC),
            Expr::u32(C_FRONTEND_WORKSPACE_MAGIC),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::ABI_VERSION),
            Expr::u32(C_FRONTEND_WORKSPACE_ABI_VERSION),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::CURRENT_PHASE),
            Expr::u32(CFrontendPhase::ResidentReady.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::REQUESTED_PHASE),
            Expr::u32(CFrontendPhase::ResidentReady.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::STATUS),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_KIND),
            Expr::u32(CFrontendCapacityDiagnosticKind::None.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_REGION),
            Expr::u32(CFrontendRegionId::Manifest.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_REQUIRED),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_CAPACITY),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::TOKEN_COUNT),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::MACRO_COUNT),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::VAST_ROW_COUNT),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::PG_EDGE_COUNT),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_COUNT),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::WORK_QUEUE_HEAD),
            Expr::u32(0),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::WORK_QUEUE_TAIL),
            Expr::u32(0),
        ),
    ];

    for region in manifest.regions() {
        init.extend(region_manifest_store_nodes(region));
    }

    vec![Node::if_then(
        Expr::eq(Expr::gid_x(), Expr::u32(0)),
        vec![
            Node::let_bind(
                "c_frontend_bootstrap_magic",
                Expr::load(C_FRONTEND_WORKSPACE_BUFFER, Expr::u32(manifest_word::MAGIC)),
            ),
            Node::if_then(
                Expr::ne(
                    Expr::var("c_frontend_bootstrap_magic"),
                    Expr::u32(C_FRONTEND_WORKSPACE_MAGIC),
                ),
                init,
            ),
        ],
    )]
}

/// GPU IR that dispatches resident C frontend phase handlers.
///
/// Only global invocation zero runs the control-plane phase machine. Data-plane
/// phase bodies may internally fan out across lanes/workgroups. If no handler
/// owns the current phase, this fragment leaves the phase unchanged.
#[must_use]
pub fn c_frontend_phase_dispatch_nodes(handlers: &[CFrontendPhaseHandler]) -> Vec<Node> {
    let mut dispatch = vec![
        Node::let_bind(
            "c_frontend_status",
            Expr::load(
                C_FRONTEND_WORKSPACE_BUFFER,
                Expr::u32(manifest_word::STATUS),
            ),
        ),
        Node::let_bind(
            "c_frontend_phase",
            Expr::load(
                C_FRONTEND_WORKSPACE_BUFFER,
                Expr::u32(manifest_word::CURRENT_PHASE),
            ),
        ),
    ];

    for handler in handlers {
        dispatch.push(Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("c_frontend_status"), Expr::u32(0)),
                Expr::eq(Expr::var("c_frontend_phase"), Expr::u32(handler.phase.id())),
            ),
            handler.body.clone(),
        ));
    }

    vec![Node::if_then(
        Expr::eq(Expr::gid_x(), Expr::u32(0)),
        dispatch,
    )]
}

/// GPU IR that validates the resident requested/current phase pair.
///
/// The guard runs before phase dispatch. It does not fabricate progress: it
/// only faults malformed resident state so later handlers cannot silently run
/// against an impossible phase-machine edge.
#[must_use]
pub fn c_frontend_phase_machine_guard_nodes() -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::gid_x(), Expr::u32(0)),
        vec![
            Node::let_bind(
                "c_frontend_guard_status",
                Expr::load(
                    C_FRONTEND_WORKSPACE_BUFFER,
                    Expr::u32(manifest_word::STATUS),
                ),
            ),
            Node::let_bind(
                "c_frontend_guard_current_phase",
                Expr::load(
                    C_FRONTEND_WORKSPACE_BUFFER,
                    Expr::u32(manifest_word::CURRENT_PHASE),
                ),
            ),
            Node::let_bind(
                "c_frontend_guard_requested_phase",
                Expr::load(
                    C_FRONTEND_WORKSPACE_BUFFER,
                    Expr::u32(manifest_word::REQUESTED_PHASE),
                ),
            ),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("c_frontend_guard_status"), Expr::u32(0)),
                    Expr::ne(
                        Expr::var("c_frontend_guard_requested_phase"),
                        Expr::var("c_frontend_guard_current_phase"),
                    ),
                ),
                vec![Node::if_then(
                    Expr::eq(c_frontend_requested_phase_valid_expr(), Expr::bool(false)),
                    c_frontend_fault_expr_nodes(
                        CFrontendCapacityDiagnosticKind::PhaseTransition,
                        CFrontendRegionId::Manifest,
                        Expr::var("c_frontend_guard_requested_phase"),
                        Expr::var("c_frontend_guard_current_phase"),
                    ),
                )],
            ),
        ],
    )]
}

/// GPU IR that advances a resident phase after a successful handler.
pub fn c_frontend_advance_phase_nodes(
    from: CFrontendPhase,
    to: CFrontendPhase,
) -> Result<Vec<Node>, CFrontendWorkspaceError> {
    validate_c_frontend_phase_transition(from, to)?;
    Ok(vec![
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::REQUESTED_PHASE),
            Expr::u32(to.id()),
        ),
        Node::let_bind(
            "c_frontend_phase_prev",
            Expr::atomic_compare_exchange(
                C_FRONTEND_WORKSPACE_BUFFER,
                Expr::u32(manifest_word::CURRENT_PHASE),
                Expr::u32(from.id()),
                Expr::u32(to.id()),
            ),
        ),
        Node::if_then(
            Expr::ne(Expr::var("c_frontend_phase_prev"), Expr::u32(from.id())),
            c_frontend_fault_nodes(
                CFrontendCapacityDiagnosticKind::PhaseTransition,
                CFrontendRegionId::Manifest,
                to.id(),
                from.id(),
            ),
        ),
    ])
}

/// GPU IR that faults the resident C frontend workspace with a structured
/// diagnostic in manifest words.
#[must_use]
pub fn c_frontend_fault_nodes(
    kind: CFrontendCapacityDiagnosticKind,
    region: CFrontendRegionId,
    required: u32,
    capacity: u32,
) -> Vec<Node> {
    c_frontend_fault_expr_nodes(kind, region, Expr::u32(required), Expr::u32(capacity))
}

fn c_frontend_fault_expr_nodes(
    kind: CFrontendCapacityDiagnosticKind,
    region: CFrontendRegionId,
    required: Expr,
    capacity: Expr,
) -> Vec<Node> {
    vec![
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_KIND),
            Expr::u32(kind.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_REGION),
            Expr::u32(region.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_REQUIRED),
            required,
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_CAPACITY),
            capacity,
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::DIAGNOSTIC_COUNT),
            Expr::u32(1),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::STATUS),
            Expr::u32(kind.id()),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(manifest_word::CURRENT_PHASE),
            Expr::u32(CFrontendPhase::Fault.id()),
        ),
    ]
}

fn c_frontend_requested_phase_valid_expr() -> Expr {
    let current = Expr::var("c_frontend_guard_current_phase");
    let requested = Expr::var("c_frontend_guard_requested_phase");
    let in_range = Expr::and(
        Expr::le(current.clone(), Expr::u32(CFrontendPhase::Fault.id())),
        Expr::le(requested.clone(), Expr::u32(CFrontendPhase::Fault.id())),
    );
    let sequential = Expr::and(
        Expr::le(current.clone(), Expr::u32(CFrontendPhase::Validate.id())),
        Expr::eq(requested.clone(), Expr::add(current.clone(), Expr::u32(1))),
    );
    let reset_after_complete = Expr::and(
        Expr::eq(current.clone(), Expr::u32(CFrontendPhase::Complete.id())),
        Expr::eq(
            requested.clone(),
            Expr::u32(CFrontendPhase::ResidentReady.id()),
        ),
    );
    let requested_fault = Expr::eq(requested, Expr::u32(CFrontendPhase::Fault.id()));
    Expr::and(
        in_range,
        Expr::or(Expr::or(sequential, reset_after_complete), requested_fault),
    )
}

fn region_manifest_store_nodes(region: CFrontendWorkspaceRegion) -> Vec<Node> {
    let base =
        manifest_word::REGION_TABLE_BASE + region.id.id() * manifest_word::REGION_TABLE_ENTRY_WORDS;
    vec![
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(base),
            Expr::u32(region.offset_words),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(base + 1),
            Expr::u32(region.words),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(base + 2),
            Expr::u32(region.record_words),
        ),
        Node::store(
            C_FRONTEND_WORKSPACE_BUFFER,
            Expr::u32(base + 3),
            Expr::u32(region.capacity_records),
        ),
    ]
}
