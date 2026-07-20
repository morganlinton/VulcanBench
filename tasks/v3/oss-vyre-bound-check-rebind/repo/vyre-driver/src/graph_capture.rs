//! Backend-neutral planning for replayable graph-capture dispatch paths.
//!
//! CUDA graphs, WGPU command replay, and future persistent-dispatch recorders
//! all need the same first step: walk a [`BindingPlan`] once, classify which
//! runtime buffers require stable input storage, which require output readback
//! storage, and how many kernel pointer arguments are needed in lowered binding
//! order. This module owns that logic so backend crates do not fork planner
//! invariants while adding API-specific capture and replay code.

use crate::binding::{BindingPlan, BindingRole};
use crate::transfer_accounting::TransferAccountingPolicy;
use crate::BackendError;

const GRAPH_CAPTURE_BINDING_ACCOUNTING: TransferAccountingPolicy =
    TransferAccountingPolicy::new("graph capture binding plan", "record a smaller graph shape");

/// Schema version for scan graph-capture edit classification evidence.
pub const SCAN_GRAPH_CAPTURE_EDIT_SCHEMA_VERSION: u32 = 1;

/// Capacity and safety plan for recording one replayable dispatch graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphCaptureBindingPlan {
    /// Device/storage entries needed for runtime input buffers. Input-output
    /// bindings are counted here because their input allocation is reused for
    /// output readback.
    pub input_device_capacity: usize,
    /// Device/storage entries needed for non-input runtime buffers. This is
    /// intentionally separate from [`Self::output_readback_capacity`] because
    /// an input-output binding needs output readback metadata but does not need
    /// a second device pointer.
    pub output_device_capacity: usize,
    /// Host/readback entries needed for bindings with an output view.
    pub output_readback_capacity: usize,
    /// Pointer arguments passed to the captured kernel in binding order.
    pub kernel_pointer_capacity: usize,
    /// Kernel pointer arguments plus the trailing launch-parameter pointer.
    pub kernel_argument_capacity: usize,
    /// True when a backend can replay a no-upload steady-state graph after the
    /// device inputs have been initialized once.
    pub resident_input_replay_safe: bool,
}

/// Scan-specific edit class that can affect graph replay safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanGraphCaptureEditKind {
    /// Resident pattern database upload or replacement.
    PatternDatabaseUpload,
    /// Haystack bytes changed between graph dispatches.
    HaystackBufferChange,
    /// Output slab size or layout changed.
    OutputSlabResize,
    /// Verifier program or verifier metadata changed.
    VerifierChange,
}

impl ScanGraphCaptureEditKind {
    /// Stable evidence label for this scan edit kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PatternDatabaseUpload => "pattern_database_upload",
            Self::HaystackBufferChange => "haystack_buffer_change",
            Self::OutputSlabResize => "output_slab_resize",
            Self::VerifierChange => "verifier_change",
        }
    }
}

/// Backend-neutral action selected for a scan graph-capture edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphCaptureEditAction {
    /// Reuse the captured graph without parameter update.
    Replay,
    /// Update graph parameters or copied input bytes without recapturing topology.
    Update,
    /// Re-record the graph because topology, pointer shape, or code changed.
    Recapture,
}

impl GraphCaptureEditAction {
    /// Stable evidence label for this capture action.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::Update => "update",
            Self::Recapture => "recapture",
        }
    }
}

/// Graph-topology stability after applying one scan edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphCaptureEditStability {
    /// Captured graph topology and pointer table shape remain valid.
    GraphStable,
    /// Captured graph topology or pointer table shape is invalidated.
    GraphBreaking,
}

impl GraphCaptureEditStability {
    /// Stable evidence label for this stability class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphStable => "graph_stable",
            Self::GraphBreaking => "graph_breaking",
        }
    }
}

/// Input facts for classifying one scan graph-capture edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanGraphCaptureEdit {
    /// Edit kind to classify.
    pub kind: ScanGraphCaptureEditKind,
    /// Previous resident artifact or buffer byte length.
    pub previous_byte_len: u64,
    /// New resident artifact or buffer byte length.
    pub next_byte_len: u64,
    /// Previous content, table, or verifier digest.
    pub previous_digest: u64,
    /// New content, table, or verifier digest.
    pub next_digest: u64,
}

impl ScanGraphCaptureEdit {
    /// Construct scan graph-capture edit facts.
    #[must_use]
    pub const fn new(
        kind: ScanGraphCaptureEditKind,
        previous_byte_len: u64,
        next_byte_len: u64,
        previous_digest: u64,
        next_digest: u64,
    ) -> Self {
        Self {
            kind,
            previous_byte_len,
            next_byte_len,
            previous_digest,
            next_digest,
        }
    }

    const fn shape_unchanged(self) -> bool {
        self.previous_byte_len == self.next_byte_len
    }

    const fn digest_unchanged(self) -> bool {
        self.previous_digest == self.next_digest
    }
}

/// Evidence emitted by scan graph-capture edit classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanGraphCaptureEditClassification {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Edit kind that was classified.
    pub edit_kind: ScanGraphCaptureEditKind,
    /// Replay, update, or recapture action.
    pub action: GraphCaptureEditAction,
    /// Whether graph topology and pointer shape remain stable.
    pub stability: GraphCaptureEditStability,
    /// Exact reason code for tests, logs, and release evidence.
    pub reason: &'static str,
    /// True when the graph can be replayed without re-recording.
    pub graph_stable: bool,
    /// True when the edit invalidates the captured graph.
    pub graph_breaking: bool,
    /// True when content changed but graph shape did not.
    pub parameter_update_required: bool,
}

impl ScanGraphCaptureEditClassification {
    /// Return true when this evidence has a valid schema, exact reason, and a
    /// self-consistent action/stability pair.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.schema_version == SCAN_GRAPH_CAPTURE_EDIT_SCHEMA_VERSION
            && !self.reason.is_empty()
            && self.graph_stable == matches!(self.stability, GraphCaptureEditStability::GraphStable)
            && self.graph_breaking
                == matches!(self.stability, GraphCaptureEditStability::GraphBreaking)
            && self.parameter_update_required
                == matches!(self.action, GraphCaptureEditAction::Update)
    }
}

/// Build a backend-neutral capture plan from a lowered binding plan.
///
/// # Errors
///
/// Returns [`BackendError::InvalidProgram`] if capacity arithmetic would
/// overflow on the host.
pub fn plan_graph_capture_bindings(
    bindings: &BindingPlan,
) -> Result<GraphCaptureBindingPlan, BackendError> {
    let mut input_device_capacity = 0usize;
    let mut output_device_capacity = 0usize;
    let mut output_readback_capacity = 0usize;
    let mut kernel_pointer_capacity = 0usize;
    let mut resident_input_replay_safe = true;

    for binding in &bindings.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }

        kernel_pointer_capacity =
            graph_capture_capacity_add(kernel_pointer_capacity, 1, "kernel pointer table")?;

        if binding.input_index.is_some() {
            input_device_capacity =
                graph_capture_capacity_add(input_device_capacity, 1, "input device table")?;
        } else {
            output_device_capacity =
                graph_capture_capacity_add(output_device_capacity, 1, "output device table")?;
        }

        if binding.output_index.is_some() {
            output_readback_capacity =
                graph_capture_capacity_add(output_readback_capacity, 1, "output readback table")?;
        }

        if binding.input_index.is_some() && binding.output_index.is_some() {
            resident_input_replay_safe = false;
        }
    }

    let kernel_argument_capacity =
        graph_capture_capacity_add(kernel_pointer_capacity, 1, "kernel argument table")?;

    Ok(GraphCaptureBindingPlan {
        input_device_capacity,
        output_device_capacity,
        output_readback_capacity,
        kernel_pointer_capacity,
        kernel_argument_capacity,
        resident_input_replay_safe,
    })
}

/// Classify one scan workload edit for replayable graph capture.
///
/// Pattern database and verifier changes are graph-breaking because they alter
/// resident scan code/data semantics. Haystack content changes with identical
/// byte length are graph-stable parameter updates. Output slab resizing is
/// graph-breaking because readback and pointer-shape assumptions change.
#[must_use]
pub const fn classify_scan_graph_capture_edit(
    edit: ScanGraphCaptureEdit,
) -> ScanGraphCaptureEditClassification {
    match edit.kind {
        ScanGraphCaptureEditKind::PatternDatabaseUpload => {
            if edit.shape_unchanged() && edit.digest_unchanged() {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Replay,
                    GraphCaptureEditStability::GraphStable,
                    "pattern_database_unchanged",
                )
            } else {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Recapture,
                    GraphCaptureEditStability::GraphBreaking,
                    "pattern_database_changed",
                )
            }
        }
        ScanGraphCaptureEditKind::HaystackBufferChange => {
            if edit.shape_unchanged() {
                if edit.digest_unchanged() {
                    scan_graph_capture_classification(
                        edit.kind,
                        GraphCaptureEditAction::Replay,
                        GraphCaptureEditStability::GraphStable,
                        "haystack_unchanged",
                    )
                } else {
                    scan_graph_capture_classification(
                        edit.kind,
                        GraphCaptureEditAction::Update,
                        GraphCaptureEditStability::GraphStable,
                        "haystack_contents_changed_same_shape",
                    )
                }
            } else {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Recapture,
                    GraphCaptureEditStability::GraphBreaking,
                    "haystack_shape_changed",
                )
            }
        }
        ScanGraphCaptureEditKind::OutputSlabResize => {
            if edit.shape_unchanged() {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Replay,
                    GraphCaptureEditStability::GraphStable,
                    "output_slab_unchanged",
                )
            } else {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Recapture,
                    GraphCaptureEditStability::GraphBreaking,
                    "output_slab_size_changed",
                )
            }
        }
        ScanGraphCaptureEditKind::VerifierChange => {
            if edit.shape_unchanged() && edit.digest_unchanged() {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Replay,
                    GraphCaptureEditStability::GraphStable,
                    "verifier_unchanged",
                )
            } else {
                scan_graph_capture_classification(
                    edit.kind,
                    GraphCaptureEditAction::Recapture,
                    GraphCaptureEditStability::GraphBreaking,
                    "verifier_changed",
                )
            }
        }
    }
}

const fn scan_graph_capture_classification(
    edit_kind: ScanGraphCaptureEditKind,
    action: GraphCaptureEditAction,
    stability: GraphCaptureEditStability,
    reason: &'static str,
) -> ScanGraphCaptureEditClassification {
    ScanGraphCaptureEditClassification {
        schema_version: SCAN_GRAPH_CAPTURE_EDIT_SCHEMA_VERSION,
        edit_kind,
        action,
        stability,
        reason,
        graph_stable: matches!(stability, GraphCaptureEditStability::GraphStable),
        graph_breaking: matches!(stability, GraphCaptureEditStability::GraphBreaking),
        parameter_update_required: matches!(action, GraphCaptureEditAction::Update),
    }
}

fn graph_capture_capacity_add(lhs: usize, rhs: usize, label: &str) -> Result<usize, BackendError> {
    GRAPH_CAPTURE_BINDING_ACCOUNTING.add_usize_capacity(lhs, rhs, label)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_scan_graph_capture_edit, graph_capture_capacity_add, plan_graph_capture_bindings,
        GraphCaptureBindingPlan, GraphCaptureEditAction, GraphCaptureEditStability,
        ScanGraphCaptureEdit, ScanGraphCaptureEditKind,
    };
    use crate::binding::{Binding, BindingPlan, BindingRole};
    use std::sync::Arc;

    fn binding(
        name: &'static str,
        slot: u32,
        role: BindingRole,
        input_index: Option<usize>,
        output_index: Option<usize>,
    ) -> Binding {
        Binding {
            name: Arc::from(name),
            binding: slot,
            buffer_index: slot as usize,
            role,
            element_size: 4,
            preferred_alignment: 4,
            element_count: 16,
            static_byte_len: Some(64),
            input_index,
            output_index,
        }
    }

    fn plan(bindings: Vec<Binding>) -> BindingPlan {
        BindingPlan {
            bindings,
            input_indices: vec![],
            output_indices: vec![],
            shared_indices: vec![],
        }
    }

    #[test]
    fn graph_capture_binding_plan_counts_distinct_device_and_readback_tables() {
        let bindings = plan(vec![
            binding("input", 0, BindingRole::Input, Some(0), None),
            binding("shared", 1, BindingRole::Shared, None, None),
            binding("output", 2, BindingRole::Output, None, Some(0)),
            binding("state", 3, BindingRole::InputOutput, Some(1), Some(1)),
        ]);

        assert_eq!(
            plan_graph_capture_bindings(&bindings)
                .expect("Fix: graph capture planning should accept normal bindings"),
            GraphCaptureBindingPlan {
                input_device_capacity: 2,
                output_device_capacity: 1,
                output_readback_capacity: 2,
                kernel_pointer_capacity: 3,
                kernel_argument_capacity: 4,
                resident_input_replay_safe: false,
            }
        );
    }

    #[test]
    fn generated_graph_capture_binding_plan_preserves_order_independent_counts() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for case_index in 0..768usize {
            let binding_count = 1 + (next_u64(&mut state) as usize % 96);
            let mut bindings = Vec::with_capacity(binding_count);
            let mut expected_input_device_capacity = 0usize;
            let mut expected_output_device_capacity = 0usize;
            let mut expected_output_readback_capacity = 0usize;
            let mut expected_kernel_pointer_capacity = 0usize;
            let mut expected_safe = true;
            let mut next_input = 0usize;
            let mut next_output = 0usize;

            for slot in 0..binding_count {
                let role_selector = (next_u64(&mut state) % 4) as u8;
                let (role, input_index, output_index) = match role_selector {
                    0 => {
                        let index = next_input;
                        next_input += 1;
                        (BindingRole::Input, Some(index), None)
                    }
                    1 => {
                        let index = next_output;
                        next_output += 1;
                        (BindingRole::Output, None, Some(index))
                    }
                    2 => {
                        let input = next_input;
                        let output = next_output;
                        next_input += 1;
                        next_output += 1;
                        expected_safe = false;
                        (BindingRole::InputOutput, Some(input), Some(output))
                    }
                    _ => (BindingRole::Shared, None, None),
                };

                if role != BindingRole::Shared {
                    expected_kernel_pointer_capacity += 1;
                    if input_index.is_some() {
                        expected_input_device_capacity += 1;
                    } else {
                        expected_output_device_capacity += 1;
                    }
                    if output_index.is_some() {
                        expected_output_readback_capacity += 1;
                    }
                }

                bindings.push(binding(
                    "generated",
                    slot as u32,
                    role,
                    input_index,
                    output_index,
                ));
            }

            let planned = plan_graph_capture_bindings(&plan(bindings))
                .expect("Fix: generated graph capture plan should fit host capacities");
            assert_eq!(
                planned,
                GraphCaptureBindingPlan {
                    input_device_capacity: expected_input_device_capacity,
                    output_device_capacity: expected_output_device_capacity,
                    output_readback_capacity: expected_output_readback_capacity,
                    kernel_pointer_capacity: expected_kernel_pointer_capacity,
                    kernel_argument_capacity: expected_kernel_pointer_capacity + 1,
                    resident_input_replay_safe: expected_safe,
                },
                "case {case_index}"
            );
        }
    }

    #[test]
    fn graph_capture_capacity_overflow_fails_loudly() {
        let error = graph_capture_capacity_add(usize::MAX, 1, "kernel argument table")
            .expect_err("Fix: graph capture capacity overflow must not wrap");
        let message = error.to_string();
        assert!(message.contains("graph capture binding plan"));
        assert!(message.contains("kernel argument table"));
        assert!(message.contains("record a smaller graph shape"));
    }

    #[test]
    fn scan_graph_capture_classifies_replay_update_and_recapture_reasons() {
        let cases = [
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::PatternDatabaseUpload,
                    4096,
                    4096,
                    11,
                    11,
                ),
                GraphCaptureEditAction::Replay,
                GraphCaptureEditStability::GraphStable,
                "pattern_database_unchanged",
            ),
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::PatternDatabaseUpload,
                    4096,
                    4096,
                    11,
                    12,
                ),
                GraphCaptureEditAction::Recapture,
                GraphCaptureEditStability::GraphBreaking,
                "pattern_database_changed",
            ),
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::HaystackBufferChange,
                    8192,
                    8192,
                    21,
                    22,
                ),
                GraphCaptureEditAction::Update,
                GraphCaptureEditStability::GraphStable,
                "haystack_contents_changed_same_shape",
            ),
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::HaystackBufferChange,
                    8192,
                    16_384,
                    21,
                    22,
                ),
                GraphCaptureEditAction::Recapture,
                GraphCaptureEditStability::GraphBreaking,
                "haystack_shape_changed",
            ),
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::OutputSlabResize,
                    1024,
                    2048,
                    31,
                    31,
                ),
                GraphCaptureEditAction::Recapture,
                GraphCaptureEditStability::GraphBreaking,
                "output_slab_size_changed",
            ),
            (
                ScanGraphCaptureEdit::new(
                    ScanGraphCaptureEditKind::VerifierChange,
                    512,
                    512,
                    41,
                    42,
                ),
                GraphCaptureEditAction::Recapture,
                GraphCaptureEditStability::GraphBreaking,
                "verifier_changed",
            ),
        ];

        for (edit, action, stability, reason) in cases {
            let classified = classify_scan_graph_capture_edit(edit);
            assert!(classified.is_complete());
            assert_eq!(classified.edit_kind, edit.kind);
            assert_eq!(classified.action, action);
            assert_eq!(classified.stability, stability);
            assert_eq!(classified.reason, reason);
        }
    }

    #[test]
    fn scan_graph_capture_same_shape_haystack_update_is_not_a_hidden_recapture() {
        let classified = classify_scan_graph_capture_edit(ScanGraphCaptureEdit::new(
            ScanGraphCaptureEditKind::HaystackBufferChange,
            65_536,
            65_536,
            100,
            101,
        ));

        assert_eq!(classified.action, GraphCaptureEditAction::Update);
        assert!(classified.graph_stable);
        assert!(!classified.graph_breaking);
        assert!(classified.parameter_update_required);
        assert_eq!(classified.reason, "haystack_contents_changed_same_shape");
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }
}
