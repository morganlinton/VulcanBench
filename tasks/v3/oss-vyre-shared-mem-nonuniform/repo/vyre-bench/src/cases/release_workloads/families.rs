use super::{ReleaseMacroFamily, SyntheticCountWorkload, SyntheticPattern};

pub(super) static CONDITION_EVAL_BATCH: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.condition_eval.1m",
    name: "Release Condition Evaluation 1M",
    description: "Bytecode-compatible condition evaluation over a 1M rule-record batch",
    tags: &["condition", "bytecode", "rules"],
    owner_crate: "vyre",
    primitive: "bytecode-compatible conditional evaluation",
    baseline: "optimized CPU rule-condition evaluator with SIMD-friendly bitmap inputs",
    metric_name: "condition_records",
    family: ReleaseMacroFamily::Condition,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::ConditionEval,
};

pub(super) static QUANTIFIED_LOOPS: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.quantified_condition_loops.1m",
    name: "Release Quantified Condition Loops 1M",
    description: "Bounded FOR-ANY, FOR-ALL, and FOR-N style condition evaluation",
    tags: &["quantifier", "loop", "predicate"],
    owner_crate: "vyre",
    primitive: "bounded quantified condition loops",
    baseline: "optimized CPU short-circuit quantified-condition evaluator",
    metric_name: "quantified_records",
    family: ReleaseMacroFamily::Condition,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::QuantifiedLoops,
};

pub(super) static STRING_BITMAP_SCATTER: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.string_bitmap_scatter.1m",
    name: "Release String Bitmap Scatter 1M",
    description: "Pattern-match bitmap scatter feeding per-rule condition evaluation",
    tags: &["string", "bitmap", "scatter"],
    owner_crate: "vyre-libs",
    primitive: "pattern-match bitmap scatter",
    baseline: "Hyperscan/ripgrep-class CPU pattern bitmap materialization",
    metric_name: "scatter_records",
    family: ReleaseMacroFamily::Scan,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::StringBitmapScatter,
};

pub(super) static OFFSET_COUNT_AGGREGATION: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.offset_count_aggregation.1m",
    name: "Release Offset Count Aggregation 1M",
    description: "String offset, length, and count aggregation without CPU-side post-processing",
    tags: &["offset", "count", "aggregation"],
    owner_crate: "vyre-libs",
    primitive: "count/offset/length aggregation",
    baseline: "SIMD CPU aggregation over sorted match streams",
    metric_name: "aggregation_records",
    family: ReleaseMacroFamily::Scan,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::OffsetCountAggregation,
};

pub(super) static ENTROPY_WINDOW: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.entropy_window.1m",
    name: "Release Entropy Window 1M",
    description: "Rolling entropy-style window predicates over a byte-statistics stream",
    tags: &["entropy", "window", "statistics"],
    owner_crate: "vyre-libs",
    primitive: "rolling entropy/window predicates",
    baseline: "SIMD CPU rolling histogram entropy implementation",
    metric_name: "entropy_records",
    family: ReleaseMacroFamily::Scan,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::EntropyWindow,
};

pub(super) static ALIAS_REACHING_DEF: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.alias_reaching_def.1m",
    name: "Release Alias Reaching Definition 1M",
    description: "Alias-aware reaching-definition predicate workload used by optimization passes",
    tags: &["alias", "reaching-def", "dataflow", "weir"],
    owner_crate: "weir",
    primitive: "Weir alias-aware reaching-definition frontier optimization",
    baseline: "Weir sparse dataflow and alias-analysis CPU baseline",
    metric_name: "alias_records",
    family: ReleaseMacroFamily::Flow,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::AliasReachingDef,
};

pub(super) static IFDS_WITNESS: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.ifds_witness.1m",
    name: "Release IFDS Witness 1M",
    description: "IFDS frontier and edge-kind predicate stage for witness extraction",
    tags: &["ifds", "witness", "dataflow", "weir"],
    owner_crate: "weir",
    primitive: "Weir IFDS reachability and witness extraction",
    baseline: "Weir IFDS CPU reachability and witness extraction",
    metric_name: "ifds_records",
    family: ReleaseMacroFamily::Flow,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::IfdsWitness,
};

pub(super) static C_AST_TRAVERSAL: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.c_ast_traversal.1m",
    name: "Release C AST Traversal 1M",
    description: "C AST node motif predicate traversal over parser-produced node buffers",
    tags: &["c", "ast", "parser"],
    owner_crate: "vyre-frontend-c",
    primitive: "C AST traversal and motif predicates",
    baseline: "tree-sitter/libclang-class CPU AST traversal baseline",
    metric_name: "ast_nodes",
    family: ReleaseMacroFamily::Parser,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::CAstTraversal,
};

pub(super) static MEGAKERNEL_QUEUE: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.megakernel_queue.1m",
    name: "Release Megakernel Queue 1M",
    description: "Persistent megakernel queue predicate workload for repeated condition batches",
    tags: &["megakernel", "queue", "runtime"],
    owner_crate: "vyre-runtime",
    primitive: "persistent megakernel queued condition batches",
    baseline: "optimized CPU batched condition evaluator",
    metric_name: "queued_records",
    family: ReleaseMacroFamily::Resident,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::MegakernelQueuedBatch,
};

pub(super) static EGRAPH_SATURATION: SyntheticCountWorkload = SyntheticCountWorkload {
    id: "release.egraph_saturation.1m",
    name: "Release Egraph Saturation 1M",
    description: "Rewrite-equivalence predicate workload for optimization saturation evidence",
    tags: &["egraph", "optimization", "rewrite"],
    owner_crate: "vyre-lower",
    primitive: "optimization rewrite saturation",
    baseline: "egg/egraph CPU saturation baseline with equivalent rewrite set",
    metric_name: "rewrite_records",
    family: ReleaseMacroFamily::Egraph,
    records: 1_048_576,
    min_speedup_x: 100.0,
    pattern: SyntheticPattern::EgraphSaturation,
};

static CONDITION_WORKLOADS: [&SyntheticCountWorkload; 2] =
    [&CONDITION_EVAL_BATCH, &QUANTIFIED_LOOPS];
static SCAN_WORKLOADS: [&SyntheticCountWorkload; 3] = [
    &STRING_BITMAP_SCATTER,
    &OFFSET_COUNT_AGGREGATION,
    &ENTROPY_WINDOW,
];
static FLOW_WORKLOADS: [&SyntheticCountWorkload; 2] = [&ALIAS_REACHING_DEF, &IFDS_WITNESS];
static PARSER_WORKLOADS: [&SyntheticCountWorkload; 1] = [&C_AST_TRAVERSAL];
static RESIDENT_WORKLOADS: [&SyntheticCountWorkload; 1] = [&MEGAKERNEL_QUEUE];
static EGRAPH_WORKLOADS: [&SyntheticCountWorkload; 1] = [&EGRAPH_SATURATION];
static GRAPH_WORKLOADS: [&SyntheticCountWorkload; 0] = [];
static MATRIX_WORKLOADS: [&SyntheticCountWorkload; 0] = [];

pub(super) fn release_macro_workloads_for_family(
    family: ReleaseMacroFamily,
) -> &'static [&'static SyntheticCountWorkload] {
    match family {
        ReleaseMacroFamily::Scan => &SCAN_WORKLOADS,
        ReleaseMacroFamily::Flow => &FLOW_WORKLOADS,
        ReleaseMacroFamily::Graph => &GRAPH_WORKLOADS,
        ReleaseMacroFamily::Parser => &PARSER_WORKLOADS,
        ReleaseMacroFamily::Egraph => &EGRAPH_WORKLOADS,
        ReleaseMacroFamily::Resident => &RESIDENT_WORKLOADS,
        ReleaseMacroFamily::Matrix => &MATRIX_WORKLOADS,
        ReleaseMacroFamily::Condition => &CONDITION_WORKLOADS,
    }
}

pub(super) fn release_macro_workloads() -> [&'static SyntheticCountWorkload; 10] {
    [
        CONDITION_WORKLOADS[0],
        SCAN_WORKLOADS[0],
        SCAN_WORKLOADS[1],
        SCAN_WORKLOADS[2],
        CONDITION_WORKLOADS[1],
        FLOW_WORKLOADS[0],
        FLOW_WORKLOADS[1],
        PARSER_WORKLOADS[0],
        RESIDENT_WORKLOADS[0],
        EGRAPH_WORKLOADS[0],
    ]
}
