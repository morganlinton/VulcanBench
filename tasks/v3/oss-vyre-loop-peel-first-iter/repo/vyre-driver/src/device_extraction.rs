//! Device-conditioned e-graph extraction helpers.
//!
//! Equality saturation is substrate-neutral: one saturated e-graph can hold
//! every proven equivalent representation of a computation. Extraction is the
//! point where a concrete device should matter. This module keeps that
//! device-conditioned choice in `vyre-driver`, so native, portable, secondary, and
//! future backends share the same extraction contract instead of each lowering
//! path inventing its own cost plumbing.

use smallvec::SmallVec;
use vyre_foundation::optimizer::eqsat::{extract_best, EClassId, EGraph, ENodeLang};

use crate::autotune_store::AutotuneRecord;
use crate::device_profile::DeviceProfile;
use crate::extraction_cost::{device_aware_cost, NodeHints};
use crate::trace_jit_policy::{decide_trace_jit_speculation, TraceJitDecision, TraceJitInputs};

/// Device context used for one extraction from a saturated e-graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionDevice<'a> {
    /// Capability profile for the target backend/device.
    pub profile: &'a DeviceProfile,
    /// Last winning autotune record for the root shape on this device.
    pub autotune_record: Option<&'a AutotuneRecord>,
    /// Recent trace-JIT counters for the same shader family.
    pub trace_jit: Option<TraceJitInputs>,
    /// Whether the current root is known hot from runtime counters.
    pub hot_path: bool,
}

impl<'a> ExtractionDevice<'a> {
    /// Build an extraction context for `profile`.
    #[must_use]
    pub const fn new(profile: &'a DeviceProfile, hot_path: bool) -> Self {
        Self {
            profile,
            autotune_record: None,
            trace_jit: None,
            hot_path,
        }
    }

    /// Attach the last winning autotune record for this device/root.
    #[must_use]
    pub const fn with_autotune_record(mut self, record: &'a AutotuneRecord) -> Self {
        self.autotune_record = Some(record);
        self
    }

    /// Attach trace-JIT counters for the same shader family.
    #[must_use]
    pub const fn with_trace_jit(mut self, counters: TraceJitInputs) -> Self {
        self.trace_jit = Some(counters);
        self
    }
}

/// Best equivalent e-node selected for one device profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceExtraction<L> {
    /// Backend id from the source [`DeviceProfile`].
    pub backend: &'static str,
    /// Whether hot-path scaling was applied.
    pub hot_path: bool,
    /// Selected e-node.
    pub node: L,
    /// Total extracted cost, including child-class costs.
    pub cost: u64,
}

/// Extract the best equivalent representation for one device profile.
#[must_use]
pub fn extract_best_for_device<L, B, H>(
    egraph: &EGraph<L>,
    root: EClassId,
    device: ExtractionDevice<'_>,
    base_cost: B,
    hint_lookup: H,
) -> Option<DeviceExtraction<L>>
where
    L: ENodeLang,
    B: Fn(&L) -> u64,
    H: Fn(&L) -> NodeHints,
{
    if root.0 as usize >= egraph.class_count() {
        return None;
    }
    let profile_cost = device_aware_cost(device.profile, device.hot_path, &base_cost, &hint_lookup);
    let cost = |node: &L| {
        let hints = hint_lookup(node);
        let cost = profile_cost(node);
        apply_context_bias(cost, extraction_bias_bps(device, hints))
    };
    extract_best(egraph, root, cost).map(|(node, cost)| DeviceExtraction {
        backend: device.profile.backend,
        hot_path: device.hot_path,
        node,
        cost,
    })
}

/// Extract best variants for several devices from the same saturated e-graph.
///
/// The e-graph is not rebuilt or re-saturated between devices; only the
/// extractor cost closure changes. This is the shared substrate needed for
/// "same saturated graph, native-optimal and portable-optimal variants" workflows.
#[must_use]
pub fn extract_best_for_devices<'a, L, B, H>(
    egraph: &EGraph<L>,
    root: EClassId,
    devices: impl IntoIterator<Item = ExtractionDevice<'a>>,
    base_cost: B,
    hint_lookup: H,
) -> SmallVec<[DeviceExtraction<L>; 4]>
where
    L: ENodeLang,
    B: Fn(&L) -> u64,
    H: Fn(&L) -> NodeHints,
{
    let mut out = SmallVec::new();
    for device in devices {
        if let Some(extracted) =
            extract_best_for_device(egraph, root, device, &base_cost, &hint_lookup)
        {
            out.push(extracted);
        }
    }
    out
}

fn extraction_bias_bps(device: ExtractionDevice<'_>, hints: NodeHints) -> u32 {
    let mut bps = 10_000u32;
    if let Some(record) = device.autotune_record {
        if hints.compile_time_constant && record.unroll > 1 {
            bps = scale_bps(bps, 8_000);
        }
        if hints.fp16_eligible && record.tile.iter().any(|dim| *dim > 1) {
            bps = scale_bps(bps, 9_500);
        }
    }
    if hints.compile_time_constant {
        if let Some(counters) = device.trace_jit {
            if matches!(
                decide_trace_jit_speculation(counters),
                TraceJitDecision::Speculate { .. }
            ) {
                bps = scale_bps(bps, 7_000);
            }
        }
    }
    bps.max(1)
}

fn scale_bps(lhs_bps: u32, rhs_bps: u32) -> u32 {
    crate::numeric::compose_basis_points_u32(
        lhs_bps,
        rhs_bps,
        "device extraction bias composition",
        "driver",
    )
}

fn apply_context_bias(cost: u64, bps: u32) -> u64 {
    if bps >= 10_000 {
        return cost;
    }
    crate::numeric::scale_u64_by_basis_points_floor_min(
        cost,
        bps,
        1,
        "device extraction context bias",
        "driver",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::optimizer::eqsat::{EChildren, EGraph};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Toy {
        Scalar,
        TensorCore,
        Specialized,
    }

    impl ENodeLang for Toy {
        fn children(&self) -> EChildren {
            EChildren::new()
        }

        fn with_children(&self, _children: &[EClassId]) -> Self {
            self.clone()
        }
    }

    fn base_cost(node: &Toy) -> u64 {
        match node {
            Toy::Scalar => 10,
            Toy::TensorCore => 30,
            Toy::Specialized => 11,
        }
    }

    fn hints(node: &Toy) -> NodeHints {
        match node {
            Toy::TensorCore => NodeHints {
                fp16_eligible: true,
                compile_time_constant: false,
            },
            Toy::Specialized => NodeHints {
                fp16_eligible: false,
                compile_time_constant: true,
            },
            Toy::Scalar => NodeHints::default(),
        }
    }

    fn equivalent_toy_graph() -> (EGraph<Toy>, EClassId) {
        let mut graph = EGraph::new();
        let scalar = graph.add(Toy::Scalar);
        let tensor = graph.add(Toy::TensorCore);
        graph.union(scalar, tensor);
        graph.rebuild();
        (graph, scalar)
    }

    fn specialized_toy_graph() -> (EGraph<Toy>, EClassId) {
        let mut graph = EGraph::new();
        let scalar = graph.add(Toy::Scalar);
        let specialized = graph.add(Toy::Specialized);
        graph.union(scalar, specialized);
        graph.rebuild();
        (graph, scalar)
    }

    #[test]
    fn conservative_profile_extracts_scalar_variant() {
        let (graph, root) = equivalent_toy_graph();
        let profile = DeviceProfile::conservative("portable");
        let extracted = extract_best_for_device(
            &graph,
            root,
            ExtractionDevice::new(&profile, true),
            base_cost,
            hints,
        )
        .expect("Fix: equivalent toy graph must extract");

        assert_eq!(extracted.backend, "portable");
        assert_eq!(extracted.node, Toy::Scalar);
        assert_eq!(extracted.cost, 5);
    }

    #[test]
    fn tensor_core_profile_extracts_fp16_variant() {
        let (graph, root) = equivalent_toy_graph();
        let mut profile = DeviceProfile::conservative("native");
        profile.supports_f16 = true;
        profile.supports_tensor_cores = true;

        let extracted = extract_best_for_device(
            &graph,
            root,
            ExtractionDevice::new(&profile, true),
            base_cost,
            hints,
        )
        .expect("Fix: equivalent toy graph must extract");

        assert_eq!(extracted.backend, "native");
        assert_eq!(extracted.node, Toy::TensorCore);
        assert_eq!(extracted.cost, 4);
    }

    #[test]
    fn several_devices_extract_from_one_saturated_graph() {
        let (graph, root) = equivalent_toy_graph();
        let portable = DeviceProfile::conservative("portable");
        let mut native = DeviceProfile::conservative("native");
        native.supports_f16 = true;
        native.supports_tensor_cores = true;

        let variants = extract_best_for_devices(
            &graph,
            root,
            [
                ExtractionDevice::new(&portable, true),
                ExtractionDevice::new(&native, true),
            ],
            base_cost,
            hints,
        );

        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].node, Toy::Scalar);
        assert_eq!(variants[1].node, Toy::TensorCore);
    }

    #[test]
    fn autotune_record_biases_compile_time_constant_variant() {
        let (graph, root) = specialized_toy_graph();
        let profile = DeviceProfile::conservative("native");
        let record = AutotuneRecord {
            workgroup_size: [128, 1, 1],
            unroll: 4,
            tile: [0, 0, 0],
            recorded_at: String::new(),
        };

        let extracted = extract_best_for_device(
            &graph,
            root,
            ExtractionDevice::new(&profile, true).with_autotune_record(&record),
            base_cost,
            hints,
        )
        .expect("Fix: equivalent toy graph must extract");

        assert_eq!(extracted.node, Toy::Specialized);
        assert_eq!(extracted.cost, 4);
    }

    #[test]
    fn trace_jit_biases_specialized_variant_when_speculation_pays() {
        let (graph, root) = specialized_toy_graph();
        let profile = DeviceProfile::conservative("native");
        let counters = TraceJitInputs {
            shader_hit_count: 64,
            prediction_confidence_bps: 10_000,
            speculative_spec_cost_ns: 1,
            miss_cost_ns: 1_000_000,
        };

        let extracted = extract_best_for_device(
            &graph,
            root,
            ExtractionDevice::new(&profile, true).with_trace_jit(counters),
            base_cost,
            hints,
        )
        .expect("Fix: equivalent toy graph must extract");

        assert_eq!(extracted.node, Toy::Specialized);
        assert_eq!(extracted.cost, 4);
    }

    #[test]
    fn missing_root_returns_no_variant() {
        let graph: EGraph<Toy> = EGraph::new();
        let profile = DeviceProfile::conservative("portable");
        let variants = extract_best_for_devices(
            &graph,
            EClassId(77),
            [ExtractionDevice::new(&profile, true)],
            base_cost,
            hints,
        );

        assert!(variants.is_empty());
    }
}
