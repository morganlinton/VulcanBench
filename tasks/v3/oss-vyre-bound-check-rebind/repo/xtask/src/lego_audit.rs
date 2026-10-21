//! `cargo_full run --bin xtask -- lego-audit`  -  deeper LEGO-block enforcement.
//!
//! Gate 1 (`cargo_full run --bin xtask -- gate1`) is the floor: loops ≤ 4 AND nodes ≤ 200
//! OR composed_fraction ≥ 60%. That's table stakes. vyre's thesis is
//! composition, so the real measurement is harder.
//!
//! This xtask runs ten stricter audits:
//!
//! 1. **No-reinvention check**  -  IR fingerprint every op body; any two
//!    ops with >80% fingerprint overlap where one doesn't invoke the
//!    other get flagged as duplication.
//! 2. **Depth-of-composition**  -  `own_nodes` vs `composed_nodes`. An op
//!    with a lot of its own nodes and few composed ones at Tier 3 is
//!    failing the LEGO pattern.
//! 3. **Primitive-coverage**  -  every Tier 2.5 primitive should have
//!    ≥ 2 callers. Orphans (0 or 1 caller) are either (a) waiting for
//!    a second consumer  -  OK for one release  -  or (b) premature
//!    promotion  -  should demote back to a private helper.
//! 4. **Cross-dialect reach-through**  -  Tier 3 dialects importing
//!    private items from sibling Tier 3 dialects. That coupling
//!    belongs in Tier 2.5; flag it.
//! 5. **Large-file advisory**  -  files over a per-file source-line
//!    review guideline are reported as notes for a split-by-responsibility
//!    review. This is advisory and never fails the audit; the hard size
//!    ceiling is `scripts/check_max_file_size.sh`.
//! 6. **Composition-chain coverage**  -  every registered op must have
//!    `print-composition` render ≥ 1 child Region, or be marked
//!    `leaf = true` in its `OpEntry`. Single-top-level Region only =
//!    inlining in disguise.
//! 7. **Trend**  -  compare per-op `composed_fraction` to the previous
//!    tag; fail CI if it regresses. The thesis is "composition gets
//!    deeper over time," not "stagnates."
//! 8. **Composability**  -  flag islands: ops with no upstream caller
//!    AND no downstream child ops. The op is dead in the registry
//!    (and frequently a sign that the writer reinvented a primitive
//!    a real caller has inline).
//! 9. **Name-stem collision**  -  ≥ 4 ops sharing the leaf-prefix stem
//!    (`matmul`, `matmul_tiled`, `matmul_strassen`, `matmul_one_level`)
//!    forces a discoverable family namespace or merge.
//! 10. **Operand-shape duplicate**  -  two ops with identical
//!     fingerprint prefix and bigram-cosine ≥ 0.55 even when below
//!     check 1's 0.88 threshold. Catches "same problem, slightly
//!     reordered" duplicates that pure cosine misses.
//!
//! Exit code 0 if every check passes. Non-zero with per-check
//! diagnostic otherwise. Intended to run in CI post-Gate 1.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

const MAX_LEGO_AUDIT_SOURCE_BYTES: u64 = 2_097_152;

use vyre::ir::{Expr, Node, Program};

use crate::dedup_report::{
    duplicate_family_report, duplicate_report_generator_command, duplicate_report_json_path,
    duplicate_severity, registered_op_duplicate_family_id, registered_op_duplicate_subject,
    registered_op_owner_lane, structural_similarity, write_duplicate_report_json,
    DuplicateEvidence, DuplicateFamilyFinding, DuplicateFamilyReport, DuplicateSubject,
};

const FINGERPRINT_SIM_THRESHOLD: f64 = 0.88;
/// Line count at which a source file is flagged for a split-by-responsibility
/// *review*. Crossing it is a guideline prompt, not a law and not a build
/// failure. The hard god-file ceiling (ratcheted, with a per-file exception
/// list) is enforced by `scripts/check_max_file_size.sh`.
const LARGE_FILE_ADVISORY_LINES: usize = 500;
const MIN_CALLERS_FOR_PRIMITIVE: usize = 2;

/// Entry point for the `lego-audit` subcommand.
pub(crate) fn run(args: &[String]) {
    let with_repo = args.iter().any(|arg| arg == "--with-repo");
    let report_only = args.iter().any(|arg| arg == "--report-only");
    let duplicate_report_json = duplicate_report_json_arg(args);
    let ops = collect_ops();
    if let Some(path) = duplicate_report_json.as_ref() {
        let prefix = if report_only {
            "lego-audit --report-only"
        } else {
            "lego-audit"
        };
        let generator_command = duplicate_report_generator_command(prefix, path);
        let report = lego_duplicate_report(&ops, &generator_command);
        if let Err(error) = write_duplicate_report_json(path, &report) {
            eprintln!(
                "Fix: lego-audit could not write duplicate family report `{}`: {error}",
                path.display()
            );
            process::exit(1);
        }
    }
    println!("=== vyre LEGO-block audit ===");
    println!("Ops audited: {}", ops.len());
    println!(
        "Repo checks: {}",
        if with_repo {
            "enabled"
        } else {
            "not requested; pass --with-repo for file-shape and trend checks"
        }
    );
    println!();

    let mut failures: usize = 0;

    failures += check_1_no_reinvention(&ops);
    failures += check_2_depth_of_composition(&ops);
    failures += check_3_primitive_coverage(&ops);
    failures += check_4_cross_dialect_reachthrough();
    if with_repo {
        failures += check_5_god_files();
    }
    failures += check_6_composition_chain_coverage(&ops);
    if with_repo {
        failures += check_7_trend(&ops);
    }
    failures += check_8_composability(&ops);
    failures += check_9_name_stem_collision(&ops);
    failures += check_10_operand_shape_duplicate(&ops);

    if !with_repo {
        println!();
        println!("Checks requiring repo context (5, 7) did not run. Fix: invoke `cargo_full run --bin xtask -- lego-audit --with-repo` from a git checkout for release gates.");
    }

    if failures > 0 {
        println!();
        println!("LEGO-block audit FAILED: {failures} finding(s). Gate 1 is the floor, this is the ceiling  -  bring composed_fraction up or extract shared pieces to Tier 2.5.");
        if !report_only {
            process::exit(1);
        }
        println!("LEGO-block audit report-only mode: findings were reported without failing the process.");
        return;
    }
    println!();
    println!("LEGO-block audit ✓");
}

fn duplicate_report_json_arg(args: &[String]) -> Option<PathBuf> {
    let Some(index) = args.iter().position(|arg| arg == "--duplicate-report-json") else {
        return None;
    };
    match duplicate_report_json_path(
        "--duplicate-report-json",
        args.get(index + 1).map(String::as_str),
        "--duplicate-report-json requires a path",
    ) {
        Ok(path) => Some(path),
        Err(error) => {
            eprintln!("Fix: {error}.");
            process::exit(1);
        }
    }
}

/// One registered op with everything the audit needs.
pub(crate) struct OpInfo {
    pub(crate) id: String,
    // Kept for future audit passes that need to re-walk the raw IR
    // (e.g. to verify that Region source_region chains are stable
    // under re-optimization). The current fingerprint/own_nodes/
    // composed_nodes/children summary is already derived from the
    // Program up-front, so downstream prints don't re-read it.
    #[allow(dead_code)]
    pub(crate) program: Program,
    pub(crate) tier: Tier,
    pub(crate) buffer_signature: Vec<String>,
    pub(crate) fingerprint: Vec<u8>,
    pub(crate) own_nodes: usize,
    pub(crate) composed_nodes: usize,
    pub(crate) children: BTreeSet<String>, // op_ids this op invokes via Region.source_region
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tier {
    T2,   // vyre-intrinsics::hardware::*
    T2_5, // vyre-primitives::*
    T3,   // vyre-libs::*
    Other,
}

fn tier_of(op_id: &str) -> Tier {
    if op_id.starts_with("vyre-intrinsics::") {
        Tier::T2
    } else if op_id.starts_with("vyre-primitives::") {
        Tier::T2_5
    } else if op_id.starts_with("vyre-libs::") {
        Tier::T3
    } else {
        Tier::Other
    }
}

pub(crate) fn collect_ops() -> Vec<OpInfo> {
    let mut ops = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        ops.push(build_info(entry.id, (entry.build)()));
    }
    for entry in vyre_primitives::harness::all_entries() {
        ops.push(build_info(entry.id, (entry.build)()));
    }
    for entry in vyre_intrinsics::harness::all_entries() {
        ops.push(build_info(entry.id, (entry.build)()));
    }
    ops
}

fn build_info(id: &'static str, program: Program) -> OpInfo {
    let tier = tier_of(id);
    let mut state = Walk::default();
    for node in program.entry() {
        walk(node, false, &mut state);
    }
    OpInfo {
        id: id.to_string(),
        buffer_signature: buffer_signature(&program),
        fingerprint: fingerprint_program(&program),
        own_nodes: state.own_nodes,
        composed_nodes: state.composed_nodes,
        children: state.children,
        program,
        tier,
    }
}

fn buffer_signature(program: &Program) -> Vec<String> {
    program
        .buffers()
        .iter()
        .map(|buffer| {
            format!(
                "binding={}:access={:?}:kind={:?}:element={:?}:count={}:output={}:live_out={}:range={:?}",
                buffer.binding(),
                buffer.access(),
                buffer.kind(),
                buffer.element(),
                buffer.count(),
                buffer.is_output(),
                buffer.is_pipeline_live_out(),
                buffer.output_byte_range(),
            )
        })
        .collect()
}

#[derive(Default)]
struct Walk {
    own_nodes: usize,
    composed_nodes: usize,
    children: BTreeSet<String>,
}

fn walk(node: &Node, inside_composed: bool, state: &mut Walk) {
    if inside_composed {
        state.composed_nodes += 1;
    } else {
        state.own_nodes += 1;
    }
    match node {
        Node::Region {
            source_region,
            body,
            generator,
        } => {
            let now_composed = inside_composed || source_region.is_some();
            // Also count `generator` as a child op-id if it matches a
            // known op id (not all generators are children, but when
            // the generator string collides with a registered op id
            // that's a strong hint).
            if source_region.is_some() && generator.as_str().contains("::") {
                state.children.insert(generator.as_str().to_string());
            }
            for child in body.iter() {
                walk(child, now_composed, state);
            }
        }
        Node::Loop { body, .. } => {
            for child in body {
                walk(child, inside_composed, state);
            }
        }
        Node::Block(children) => {
            for child in children {
                walk(child, inside_composed, state);
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for child in then {
                walk(child, inside_composed, state);
            }
            for child in otherwise {
                walk(child, inside_composed, state);
            }
        }
        _ => {}
    }
}

/// Build a compact byte sequence representing the node-kind tree
/// structure of a Program's body. Two programs with identical
/// structural shape produce identical fingerprints; one-byte edits
/// produce minor differences. Used for check 1 similarity scoring.
fn fingerprint_program(program: &Program) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    for node in program.entry() {
        fingerprint_node(node, &mut out);
    }
    out
}

fn fingerprint_node(node: &Node, out: &mut Vec<u8>) {
    match node {
        Node::Let { value, .. } => {
            out.push(0x01);
            fingerprint_expr(value, out);
        }
        Node::Assign { value, .. } => {
            out.push(0x02);
            fingerprint_expr(value, out);
        }
        Node::Store { index, value, .. } => {
            out.push(0x03);
            fingerprint_expr(index, out);
            fingerprint_expr(value, out);
        }
        Node::If {
            cond,
            then,
            otherwise,
            ..
        } => {
            out.push(0x04);
            fingerprint_expr(cond, out);
            out.push(0xFE);
            for n in then {
                fingerprint_node(n, out);
            }
            out.push(0xFF);
            for n in otherwise {
                fingerprint_node(n, out);
            }
            out.push(0xFF);
        }
        Node::Loop { from, to, body, .. } => {
            out.push(0x05);
            fingerprint_expr(from, out);
            fingerprint_expr(to, out);
            out.push(0xFE);
            for n in body {
                fingerprint_node(n, out);
            }
            out.push(0xFF);
        }
        Node::Return => out.push(0x06),
        Node::Block(nodes) => {
            out.push(0x07);
            for n in nodes {
                fingerprint_node(n, out);
            }
            out.push(0xFF);
        }
        Node::Barrier {
            ordering: vyre::memory_model::MemoryOrdering::SeqCst,
        } => out.push(0x08),
        Node::Region {
            source_region,
            body,
            generator,
        } => {
            out.push(0x09);
            if source_region.is_some() {
                out.extend_from_slice(&fingerprint_name(generator.as_str()));
            } else {
                for n in body.iter() {
                    fingerprint_node(n, out);
                }
            }
            out.push(0xFF);
        }
        Node::IndirectDispatch { .. } => out.push(0x0A),
        Node::AsyncLoad { offset, size, .. } => {
            out.push(0x0B);
            fingerprint_expr(offset, out);
            fingerprint_expr(size, out);
        }
        Node::AsyncStore { offset, size, .. } => {
            out.push(0x0C);
            fingerprint_expr(offset, out);
            fingerprint_expr(size, out);
        }
        Node::AsyncWait { .. } => out.push(0x0D),
        Node::Trap { address, .. } => {
            out.push(0x0E);
            fingerprint_expr(address, out);
        }
        Node::Resume { .. } => out.push(0x0F),
        _ => out.push(0x80),
    }
}

fn fingerprint_expr(expr: &Expr, out: &mut Vec<u8>) {
    match expr {
        Expr::LitU32(value) => {
            out.push(0x21);
            out.push(literal_bucket_u32(*value));
        }
        Expr::LitI32(value) => {
            out.push(0x22);
            out.push(literal_bucket_u32(*value as u32));
        }
        Expr::LitF32(value) => {
            out.push(0x23);
            out.push(literal_bucket_u32(value.to_bits()));
        }
        Expr::LitBool(value) => {
            out.push(0x24);
            out.push(u8::from(*value));
        }
        Expr::Var(_) => out.push(0x25),
        Expr::Load { index, .. } => {
            out.push(0x26);
            fingerprint_expr(index, out);
        }
        Expr::BufLen { .. } => out.push(0x27),
        Expr::InvocationId { axis } => {
            out.push(0x28);
            out.push(*axis);
        }
        Expr::WorkgroupId { axis } => {
            out.push(0x29);
            out.push(*axis);
        }
        Expr::LocalId { axis } => {
            out.push(0x2A);
            out.push(*axis);
        }
        Expr::BinOp { op, left, right } => {
            out.push(0x2B);
            out.push(fingerprint_name(&format!("bin::{op:?}"))[0]);
            fingerprint_expr(left, out);
            fingerprint_expr(right, out);
        }
        Expr::UnOp { op, operand } => {
            out.push(0x2C);
            out.push(fingerprint_name(&format!("un::{op:?}"))[0]);
            fingerprint_expr(operand, out);
        }
        Expr::Call { op_id, args } => {
            out.push(0x2D);
            out.push(fingerprint_name(op_id.as_str())[0]);
            for arg in args {
                fingerprint_expr(arg, out);
            }
            out.push(0xFD);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            out.push(0x2E);
            fingerprint_expr(cond, out);
            fingerprint_expr(true_val, out);
            fingerprint_expr(false_val, out);
        }
        Expr::Cast { target, value } => {
            out.push(0x2F);
            out.push(fingerprint_name(&format!("cast::{target:?}"))[0]);
            fingerprint_expr(value, out);
        }
        Expr::Fma { a, b, c } => {
            out.push(0x30);
            fingerprint_expr(a, out);
            fingerprint_expr(b, out);
            fingerprint_expr(c, out);
        }
        Expr::Atomic {
            op,
            index,
            expected,
            value,
            ordering,
            ..
        } => {
            out.push(0x31);
            out.push(fingerprint_name(&format!("atomic::{op:?}::{ordering:?}"))[0]);
            fingerprint_expr(index, out);
            if let Some(expected) = expected.as_deref() {
                fingerprint_expr(expected, out);
            }
            out.push(0xFC);
            fingerprint_expr(value, out);
        }
        Expr::SubgroupBallot { cond } => {
            out.push(0x32);
            fingerprint_expr(cond, out);
        }
        Expr::SubgroupShuffle { value, lane } => {
            out.push(0x33);
            fingerprint_expr(value, out);
            fingerprint_expr(lane, out);
        }
        Expr::SubgroupReduce { value, .. } => {
            out.push(0x34);
            fingerprint_expr(value, out);
        }
        Expr::SubgroupLocalId => out.push(0x35),
        Expr::SubgroupSize => out.push(0x36),
        Expr::Opaque(extension) => {
            out.push(0x37);
            out.push(extension.stable_fingerprint()[0]);
        }
        _ => out.push(0xBF),
    }
}

fn literal_bucket_u32(value: u32) -> u8 {
    match value {
        0 => 0,
        1 => 1,
        2..=4 => 2,
        5..=31 => 3,
        32..=255 => 4,
        256..=4096 => 5,
        _ => 6,
    }
}

fn fingerprint_name(name: &str) -> [u8; 4] {
    let mut hash = 0x811C_9DC5u32;
    for byte in name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash.to_le_bytes()
}

/// Check 1: flag pairs of ops with near-identical fingerprints whose
/// Region chains don't indicate one calls the other.
///
/// Uses bigram-frequency cosine similarity  -  captures ordered
/// structure, not just node-kind sets.
fn check_1_no_reinvention(ops: &[OpInfo]) -> usize {
    println!("[1/10] No-reinvention check (bigram cosine ≥ {FINGERPRINT_SIM_THRESHOLD:.2})");
    let pairs = no_reinvention_pairs(ops);
    for (sim, a, b) in &pairs {
        println!(
            "  ✗ reinvention: `{}` and `{}` are {:.0}% structurally similar (cross-dialect) but neither composes the other. Extract the shared body into a Tier 2.5 primitive.",
            a.id,
            b.id,
            sim * 100.0
        );
    }
    if pairs.is_empty() {
        println!("  ✓ no cross-dialect duplication");
    }
    pairs.len()
}

fn no_reinvention_pairs(ops: &[OpInfo]) -> Vec<(f64, &OpInfo, &OpInfo)> {
    let mut pairs = Vec::new();
    let mut reported: BTreeSet<(String, String)> = BTreeSet::new();
    for (i, a) in ops.iter().enumerate() {
        if is_internal_phase_op(&a.id) {
            continue;
        }
        // Only compare NON-TRIVIAL ops  -  trivial kernels share the
        // same "single invocation, loop, store" skeleton and their
        // structural similarity is expected. The audit targets ops
        // with real body content.
        if a.fingerprint.len() < 40 {
            continue;
        }
        for b in ops.iter().skip(i + 1) {
            if is_internal_phase_op(&b.id) {
                continue;
            }
            // The "extract to Tier 2.5" remedy only applies when a higher
            // tier is reinventing substrate work. Similarity among two
            // primitives may indicate a future lower-level helper, but it is
            // not a Tier-3 LEGO violation and should not fail this audit.
            if a.tier != Tier::T3 && b.tier != Tier::T3 {
                continue;
            }
            if b.fingerprint.len() < 40 {
                continue;
            }
            if a.children.contains(&b.id) || b.children.contains(&a.id) {
                continue;
            }
            if a.children.iter().any(|child| b.children.contains(child)) {
                continue;
            }
            let sim = structural_similarity(&a.fingerprint, &b.fingerprint);
            if sim < FINGERPRINT_SIM_THRESHOLD {
                continue;
            }
            // Skip comparisons inside the same sub-dialect (math::*
            // vs math::* is often legitimate  -  same loop pattern over
            // same data type, different semantics).
            if same_subdialect(&a.id, &b.id) {
                continue;
            }
            let key = if a.id < b.id {
                (a.id.clone(), b.id.clone())
            } else {
                (b.id.clone(), a.id.clone())
            };
            if !reported.insert(key) {
                continue;
            }
            pairs.push((sim, a, b));
        }
    }
    pairs
}

fn is_internal_phase_op(id: &str) -> bool {
    const PHASE_MARKERS: &[&str] = &[
        "::consumer_",
        "::hidden_projection",
        "::output_projection",
        "::softmax_stats",
        "::weight_write",
        "::statement_pass",
        "::classify_at_pos",
        "::node_shape_pass",
        "::node_classification_pass",
        "::node_edge_pass",
        ".scope",
        ".decl",
        ".identifier_intern",
        "::v_cycle_phase",
        "::power_iteration_phase",
    ];
    PHASE_MARKERS.iter().any(|marker| id.contains(marker))
}

/// Two op ids share a sub-dialect when their first TWO `::` segments
/// match. `vyre-libs::math::square` and `vyre-libs::math::broadcast`
/// both live under `vyre-libs::math`, so structural similarity there
/// is expected (same shape of elementwise unary op).
fn same_subdialect(a: &str, b: &str) -> bool {
    let a_prefix: Vec<&str> = a.split("::").take(3).collect();
    let b_prefix: Vec<&str> = b.split("::").take(3).collect();
    a_prefix.len() >= 3 && b_prefix.len() >= 3 && a_prefix[..2] == b_prefix[..2]
}

/// Check 2: per-op composition depth  -  for Tier 3 ops, composed_nodes
/// should dominate own_nodes.
fn check_2_depth_of_composition(ops: &[OpInfo]) -> usize {
    let mut flagged = 0usize;
    println!("[2/10] Depth-of-composition (Tier 3 ops should have composed_nodes ≥ own_nodes)");
    for op in ops {
        if op.tier != Tier::T3 {
            continue;
        }
        if is_internal_phase_op(&op.id) {
            continue;
        }
        let total = op.own_nodes + op.composed_nodes;
        if total < 20 {
            continue; // Small ops are allowed to be flat.
        }
        if op.composed_nodes < op.own_nodes {
            println!(
                "  ✗ {} Tier 3 op has own={} composed={}  -  inlining primitive work. Wrap sub-bodies in region::wrap_child(<primitive_id>, ...).",
                op.id, op.own_nodes, op.composed_nodes
            );
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  ✓ Tier 3 ops compose more than they inline");
    }
    flagged
}

/// Check 3: every Tier 2.5 primitive needs ≥ 2 callers.
fn check_3_primitive_coverage(ops: &[OpInfo]) -> usize {
    let mut flagged = 0usize;
    println!(
        "[3/10] Primitive coverage (Tier 2.5 primitives need ≥ {MIN_CALLERS_FOR_PRIMITIVE} callers)"
    );
    let mut caller_counts: HashMap<String, usize> = HashMap::new();
    for op in ops {
        for child in &op.children {
            if tier_of(child) == Tier::T2_5 {
                *caller_counts.entry(child.clone()).or_insert(0) += 1;
            }
        }
    }
    for op in ops {
        if op.tier != Tier::T2_5 {
            continue;
        }
        let callers = caller_counts.get(&op.id).copied().unwrap_or(0);
        if callers < MIN_CALLERS_FOR_PRIMITIVE {
            println!(
                "  ⚠ {} Tier 2.5 primitive has only {} caller(s). Either attract a second caller this cycle or demote back to a private helper in its owning dialect.",
                op.id, callers
            );
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  ✓ every Tier 2.5 primitive has ≥ {MIN_CALLERS_FOR_PRIMITIVE} callers");
    }
    flagged
}

/// Check 6: composition-chain coverage  -  every non-leaf op should have
/// at least one child Region with a `source_region` pointing at
/// another registered op. Ops that explicitly declare `leaf = true`
/// are exempt (future OpEntry field).
fn check_6_composition_chain_coverage(ops: &[OpInfo]) -> usize {
    let mut flagged = 0usize;
    println!("[6/10] Composition-chain coverage (non-leaf ops must have ≥ 1 child Region with source_region)");
    for op in ops {
        // Tier 2 intrinsics and Tier 2.5 primitives are leaves unless
        // their own bodies choose to compose deeper primitives.
        if matches!(op.tier, Tier::T2 | Tier::T2_5) {
            continue;
        }
        if is_internal_phase_op(&op.id) {
            continue;
        }
        // Tiny ops are trivially allowed to be flat.
        if op.own_nodes + op.composed_nodes < 20 {
            continue;
        }
        if op.children.is_empty() {
            println!(
                "  ⚠ {} has no registered child Regions  -  either mark it a leaf primitive or wrap inlined sub-bodies via region::wrap_child(<child_op_id>, ...).",
                op.id
            );
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  ✓ every non-leaf op names at least one child op in its Region chain");
    }
    flagged
}

/// Walk `vyre-libs/src/<dialect>/**/*.rs`; flag any `use` or `pub use`
/// path reaching into `vyre_libs::<other_dialect>::...` or
/// `crate::<other_dialect>::...` across a dialect
/// boundary. Cross-dialect coupling means the shared piece belongs in
/// Tier 2.5 (`vyre-primitives`), not duplicated or imported sideways.
///
/// The check is structural  -  it parses Rust use trees with `syn`, so grouped
/// imports, aliases, globs, and `pub use` are audited consistently without
/// relying on line-oriented grep.
///
/// CRITIQUE_VISION_ALIGNMENT_2026-04-23 V5 was precisely this category:
/// `security-analysis-consumer::emit` reached into
/// `vyre_libs::security::topology::match_order` for generic byte-range
/// ordering. V5's hoist into `vyre_libs::range_ordering` and this
/// automated check keep that coupling from returning.
fn check_4_cross_dialect_reachthrough() -> usize {
    println!("[4/10] Cross-dialect reach-through (Tier 3 dialects must not import private items from sibling Tier 3 dialects)");
    let libs_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("vyre-libs").join("src"));
    let Some(libs_root) = libs_root.filter(|p| p.is_dir()) else {
        println!(
            "  ⚠ vyre-libs/src not reachable from xtask. Fix: invoke from the workspace root."
        );
        return 0;
    };
    let (dialects, list_errors) = list_dialect_dirs(&libs_root);
    if !list_errors.is_empty() {
        for error in &list_errors {
            println!("  ✗ {error}");
        }
        return list_errors.len();
    }
    if dialects.len() < 2 {
        println!("  ✓ fewer than 2 dialects present; nothing to cross.");
        return 0;
    }
    let mut flagged = 0usize;
    for dialect in &dialects {
        let dialect_name = dialect.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let mut stack = vec![dialect.clone()];
        while let Some(dir) = stack.pop() {
            let read_dir = match std::fs::read_dir(&dir) {
                Ok(read_dir) => read_dir,
                Err(error) => {
                    println!(
                        "  ✗ {}: failed to read dialect directory: {error}. Fix: make the checked source tree fully readable.",
                        dir.display()
                    );
                    flagged += 1;
                    continue;
                }
            };
            for entry in read_dir {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        println!(
                            "  ✗ {}: failed to read dialect directory entry: {error}. Fix: make the checked source tree fully readable.",
                            dir.display()
                        );
                        flagged += 1;
                        continue;
                    }
                };
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let text = match read_text_bounded(&path) {
                    Ok(text) => text,
                    Err(error) => {
                        println!(
                            "  ✗ {}: failed to read Rust source for reach-through audit: {error}. Fix: make the checked source tree fully readable.",
                            path.display()
                        );
                        flagged += 1;
                        continue;
                    }
                };
                let Ok(file) = syn::parse_file(&text) else {
                    println!(
                        "  ✗ {}/{}: failed to parse Rust source for reach-through audit. Fix: keep checked-in Rust source syntactically parseable.",
                        dialect_name,
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                    );
                    flagged += 1;
                    continue;
                };
                for use_path in collect_use_paths(&file) {
                    for other in &dialects {
                        let other_name = other.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if other_name == dialect_name || other_name.is_empty() {
                            continue;
                        }
                        if use_path.imports_dialect(other_name) {
                            println!(
                                "  ✗ {}/{} line {}: `{}` → imports `{other_name}` dialect privately. \
                                 Fix: hoist the shared piece into vyre-primitives, or route via a \
                                 public re-export at crate root.",
                                dialect_name,
                                path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("?"),
                                use_path.line,
                                use_path.segments.join("::")
                            );
                            flagged += 1;
                        }
                    }
                }
            }
        }
    }
    if flagged == 0 {
        println!("  ✓ no Tier-3 dialect imports another Tier-3 dialect privately");
    }
    flagged
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UsePath {
    segments: Vec<String>,
    line: usize,
}

impl UsePath {
    fn imports_dialect(&self, other_name: &str) -> bool {
        matches!(
            self.segments.as_slice(),
            [first, second, ..]
                if (first == "crate" || first == "vyre_libs") && second == other_name
        )
    }
}

fn collect_use_paths(file: &syn::File) -> Vec<UsePath> {
    let mut collector = UsePathCollector::default();
    syn::visit::visit_file(&mut collector, file);
    collector.paths
}

#[derive(Default)]
struct UsePathCollector {
    paths: Vec<UsePath>,
}

impl<'ast> syn::visit::Visit<'ast> for UsePathCollector {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        collect_use_tree(&item.tree, &mut Vec::new(), &mut self.paths);
    }
}

fn collect_use_tree(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<UsePath>) {
    use syn::spanned::Spanned;

    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let mut segments = prefix.clone();
            segments.push(name.ident.to_string());
            out.push(UsePath {
                segments,
                line: name.span().start().line,
            });
        }
        syn::UseTree::Rename(rename) => {
            let mut segments = prefix.clone();
            segments.push(rename.ident.to_string());
            out.push(UsePath {
                segments,
                line: rename.span().start().line,
            });
        }
        syn::UseTree::Glob(glob) => {
            let mut segments = prefix.clone();
            segments.push("*".to_string());
            out.push(UsePath {
                segments,
                line: glob.span().start().line,
            });
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, prefix, out);
            }
        }
    }
}

fn list_dialect_dirs(root: &std::path::Path) -> (Vec<std::path::PathBuf>, Vec<String>) {
    let read_dir = match std::fs::read_dir(root) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            return (
                Vec::new(),
                vec![format!(
                    "{}: failed to read dialect root: {error}. Fix: make vyre-libs/src fully readable.",
                    root.display()
                )],
            );
        }
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(format!(
                    "{}: failed to read dialect root entry: {error}. Fix: make vyre-libs/src fully readable.",
                    root.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip non-dialect dirs: region, tensor_ref, builder, buffer_names,
        // descriptor are shared utility modules at crate root; everything
        // else under src/ is a domain dialect.
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if matches!(
            name,
            "region" | "tensor_ref" | "builder" | "buffer_names" | "descriptor"
        ) {
            continue;
        }
        out.push(path);
    }
    (out, errors)
}

fn check_5_god_files() -> usize {
    println!(
        "[5/10] Large-file advisory (files over {LARGE_FILE_ADVISORY_LINES} lines flagged for split-by-responsibility review; non-blocking)"
    );
    let Some(root) = workspace_root() else {
        // A missing workspace root is a real environment failure, not a
        // size advisory, so it still fails the audit.
        println!("  ✗ workspace root not reachable from xtask. Fix: run from the vyre workspace checkout.");
        return 1;
    };

    let mut advisories = 0usize;
    let mut errors = 0usize;
    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | "target" | "target-codex" | "target-fusion-fix"
            )
        })
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                println!(
                    "  ✗ walkdir failed while scanning source files: {error}. Fix: make the checked source tree fully readable."
                );
                errors += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = match read_text_bounded(path) {
            Ok(text) => text,
            Err(error) => {
                println!(
                    "  ✗ {} could not be read for the large-file advisory: {error}. Fix: make the checked source tree fully readable.",
                    path.strip_prefix(&root).unwrap_or(path).display()
                );
                errors += 1;
                continue;
            }
        };
        let line_count = text.lines().count();
        if line_count > LARGE_FILE_ADVISORY_LINES {
            println!(
                "  • {} has {line_count} lines. Review: does this file carry more than one responsibility? If so, split it (advisory, not a failure).",
                path.strip_prefix(&root).unwrap_or(path).display()
            );
            advisories += 1;
        }
    }
    if advisories == 0 {
        println!("  ✓ no Rust source file is over the {LARGE_FILE_ADVISORY_LINES}-line review guideline");
    } else {
        println!(
            "  • {advisories} file(s) over the {LARGE_FILE_ADVISORY_LINES}-line guideline flagged for review (non-blocking)"
        );
    }
    // Only genuine I/O errors fail this check; the size guideline is advisory.
    errors
}

fn read_text_bounded(path: &std::path::Path) -> io::Result<String> {
    let mut reader = std::fs::File::open(path)?.take(MAX_LEGO_AUDIT_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_LEGO_AUDIT_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_LEGO_AUDIT_SOURCE_BYTES} byte lego audit read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

fn check_7_trend(ops: &[OpInfo]) -> usize {
    println!("[7/10] Composition trend (current composed_fraction must not regress from previous tag baseline)");
    let Some(root) = workspace_root() else {
        println!("  ✗ workspace root not reachable from xtask. Fix: run from the vyre workspace checkout.");
        return 1;
    };
    let Some(tag) = previous_tag(&root) else {
        println!("  ✓ no previous git tag found; trend check has no baseline");
        return 0;
    };
    let Some(previous) = previous_composition_baseline(&root, &tag) else {
        println!(
            "  ✗ previous tag `{tag}` has no audits/lego-composition.tsv baseline. Fix: generate and commit the baseline before cutting the next tag."
        );
        return 1;
    };

    let current = composition_fractions(ops);
    let mut flagged = 0usize;
    for (op_id, old_fraction) in previous {
        let Some(new_fraction) = current.get(&op_id) else {
            continue;
        };
        if *new_fraction + f64::EPSILON < old_fraction {
            println!(
                "  ✗ {op_id} composed_fraction regressed from {:.1}% to {:.1}%. Fix: restore Region composition or extract shared work to Tier 2.5.",
                old_fraction * 100.0,
                new_fraction * 100.0
            );
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  ✓ no composed_fraction regressions against `{tag}`");
    }
    flagged
}

fn workspace_root() -> Option<std::path::PathBuf> {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
}

fn composition_fractions(ops: &[OpInfo]) -> BTreeMap<String, f64> {
    ops.iter()
        .map(|op| {
            let total = op.own_nodes + op.composed_nodes;
            let fraction = if total == 0 {
                1.0
            } else {
                op.composed_nodes as f64 / total as f64
            };
            (op.id.clone(), fraction)
        })
        .collect()
}

fn previous_tag(root: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0", "HEAD^"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let tag = String::from_utf8(output.stdout).ok()?;
    let tag = tag.trim();
    (!tag.is_empty()).then(|| tag.to_string())
}

fn previous_composition_baseline(
    root: &std::path::Path,
    tag: &str,
) -> Option<BTreeMap<String, f64>> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{tag}:audits/lego-composition.tsv")])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let Some(op_id) = cols.next() else {
            continue;
        };
        let Some(fraction) = cols.next().and_then(|raw| raw.parse::<f64>().ok()) else {
            continue;
        };
        out.insert(op_id.to_string(), fraction);
    }
    Some(out)
}

// ============================================================
// Check 8: composability  -  flag islands.
// ============================================================
//
// An op O is an "island" when no other op composes it AND O composes
// nothing of its own. Islands fail the LEGO thesis: they are leaves
// with no upstream consumer, which means either (a) they were shipped
// on speculation and never wired in, or (b) they reinvent something a
// caller already has inline. Both cases want the user to look.
//
// Tier-2 hardware intrinsics are exempt  -  they are by definition
// terminal leaves consumed by Tier 2.5+. Same for ops marked as
// internal phases.

const ISLAND_MIN_NODES: usize = 12;

fn check_8_composability(ops: &[OpInfo]) -> usize {
    println!("[8/10] Composability (every non-leaf op must be composed by ≥ 1 caller OR compose ≥ 1 child op)");
    let mut callers: HashMap<String, usize> = HashMap::new();
    for op in ops {
        for child in &op.children {
            *callers.entry(child.clone()).or_insert(0) += 1;
        }
    }
    let mut flagged = 0usize;
    for op in ops {
        if op.tier == Tier::T2 {
            continue;
        }
        if is_internal_phase_op(&op.id) {
            continue;
        }
        if op.own_nodes + op.composed_nodes < ISLAND_MIN_NODES {
            continue;
        }
        let upstream = callers.get(&op.id).copied().unwrap_or(0);
        let downstream = op.children.len();
        if upstream == 0 && downstream == 0 {
            println!(
                "  ⚠ {} is an island: {} upstream caller(s), {} child op(s), {} total nodes. Fix: either wire it as a child of a caller, or wrap its body via region::wrap_child(<existing_primitive>, ...).",
                op.id,
                upstream,
                downstream,
                op.own_nodes + op.composed_nodes
            );
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  ✓ no island ops");
    }
    flagged
}

// ============================================================
// Check 9: name-stem collision  -  discoverability.
// ============================================================
//
// When N ops share a stem (`matmul`, `matmul_tiled`, `matmul_strassen`,
// `matmul_one_level`), a writer searching for "matmul" sees a wall of
// near-synonyms. The gate forces either (a) a discoverable family name
// (e.g. `matmul::tiled`, `matmul::strassen` namespacing), (b) merging
// near-duplicates, or (c) acknowledging the family with an explicit
// allowlist entry. Threshold: ≥ 4 ops sharing the leaf-prefix stem.

const STEM_COLLISION_MIN: usize = 4;

fn check_9_name_stem_collision(ops: &[OpInfo]) -> usize {
    println!("[9/10] Name-stem collision (≥ {STEM_COLLISION_MIN} ops sharing a leaf-prefix stem)");
    let mut buckets: HashMap<String, Vec<String>> = HashMap::new();
    for op in ops {
        if is_internal_phase_op(&op.id) {
            continue;
        }
        let leaf = op.id.rsplit("::").next().unwrap_or(&op.id);
        let stem = leaf_stem(leaf);
        if stem.is_empty() {
            continue;
        }
        buckets
            .entry(stem.to_string())
            .or_default()
            .push(op.id.clone());
    }
    let mut flagged = 0usize;
    let mut keys: Vec<&String> = buckets.keys().collect();
    keys.sort();
    for stem in keys {
        let ids = &buckets[stem];
        if ids.len() < STEM_COLLISION_MIN {
            continue;
        }
        // Skip when every op in the stem already lives in its own
        // namespace segment  -  that means the family is already
        // explicit (e.g. matmul::tiled, matmul::strassen).
        if ids
            .iter()
            .all(|id| id.contains(&format!("::{stem}::")) || id.ends_with(&format!("::{stem}")))
        {
            continue;
        }
        println!(
            "  ⚠ {} ops share leaf-stem `{stem}`: {}. Fix: namespace the family (e.g. `{stem}::tiled`, `{stem}::strassen`), merge near-duplicates, or add a stem allowlist entry.",
            ids.len(),
            ids.join(", ")
        );
        flagged += 1;
    }
    if flagged == 0 {
        println!("  ✓ no leaf-stem collisions ≥ {STEM_COLLISION_MIN}");
    }
    flagged
}

/// Reduce a leaf identifier to its discoverability stem: drop the
/// trailing `_<suffix>` segment so `matmul`, `matmul_tiled`,
/// `matmul_strassen`, `matmul_one_level` all map to `matmul`.
fn leaf_stem(leaf: &str) -> &str {
    match leaf.find('_') {
        Some(idx) => &leaf[..idx],
        None => leaf,
    }
}

// ============================================================
// Check 10: operand-shape duplicate  -  catches false negatives of check 1.
// ============================================================
//
// Check 1 fires when bigram-cosine ≥ 0.88. False negatives slip when
// two ops share the same operand-type tuple AND the same fingerprint
// prefix (the first ~16 bytes of the IR-shape fingerprint, which
// captures the entry node-kind sequence). These are the "same
// problem, slightly reordered" duplicates that bigram cosine misses.

const PREFIX_LEN: usize = 16;
const OPERAND_DUP_MIN_COSINE: f64 = 0.55;

fn check_10_operand_shape_duplicate(ops: &[OpInfo]) -> usize {
    println!(
        "[10/10] Operand-shape duplicate (same fingerprint prefix + cosine ≥ {OPERAND_DUP_MIN_COSINE:.2})"
    );
    let pairs = operand_shape_duplicate_pairs(ops);
    for (cos, a, b) in &pairs {
        println!(
            "  ⚠ shape-duplicate: `{}` and `{}` share fingerprint prefix and {:.0}% cosine. Fix: confirm the two ops are doing distinct work, or extract the shared body to vyre-primitives.",
            a.id,
            b.id,
            cos * 100.0
        );
    }
    if pairs.is_empty() {
        println!("  ✓ no operand-shape duplicates");
    }
    pairs.len()
}

fn operand_shape_duplicate_pairs(ops: &[OpInfo]) -> Vec<(f64, &OpInfo, &OpInfo)> {
    let mut buckets: HashMap<Vec<u8>, Vec<&OpInfo>> = HashMap::new();
    for op in ops {
        if is_internal_phase_op(&op.id) {
            continue;
        }
        if op.fingerprint.len() < PREFIX_LEN {
            continue;
        }
        let prefix: Vec<u8> = op.fingerprint[..PREFIX_LEN].to_vec();
        buckets.entry(prefix).or_default().push(op);
    }
    let mut pairs = Vec::new();
    let mut reported: BTreeSet<(String, String)> = BTreeSet::new();
    for ops_in_bucket in buckets.values() {
        if ops_in_bucket.len() < 2 {
            continue;
        }
        for (i, a) in ops_in_bucket.iter().enumerate() {
            for b in ops_in_bucket.iter().skip(i + 1) {
                if a.children.contains(&b.id) || b.children.contains(&a.id) {
                    continue;
                }
                if same_subdialect(&a.id, &b.id) {
                    continue;
                }
                let cos = structural_similarity(&a.fingerprint, &b.fingerprint);
                if cos < OPERAND_DUP_MIN_COSINE {
                    continue;
                }
                let key = if a.id < b.id {
                    (a.id.clone(), b.id.clone())
                } else {
                    (b.id.clone(), a.id.clone())
                };
                if !reported.insert(key) {
                    continue;
                }
                pairs.push((cos, *a, *b));
            }
        }
    }
    pairs
}

fn lego_duplicate_report(ops: &[OpInfo], generator_command: &str) -> DuplicateFamilyReport {
    let mut families = Vec::new();
    families.extend(
        no_reinvention_pairs(ops)
            .into_iter()
            .map(|(score, left, right)| {
                lego_duplicate_family("lego-audit:no-reinvention", score, left, right)
            }),
    );
    families.extend(
        operand_shape_duplicate_pairs(ops)
            .into_iter()
            .map(|(score, left, right)| {
                lego_duplicate_family("lego-audit:operand-shape", score, left, right)
            }),
    );
    duplicate_family_report(
        generator_command,
        "registered-op-lego-audit",
        families,
    )
}

fn lego_duplicate_family(
    detector: &str,
    score: f64,
    left: &OpInfo,
    right: &OpInfo,
) -> DuplicateFamilyFinding {
    DuplicateFamilyFinding {
        family_id: registered_op_duplicate_family_id(&left.id, &right.id),
        detector: detector.to_string(),
        severity: duplicate_severity(score),
        score,
        left: lego_duplicate_subject(left),
        right: lego_duplicate_subject(right),
        import_owner: if left.tier <= right.tier {
            registered_op_owner_lane(&left.id).to_string()
        } else {
            registered_op_owner_lane(&right.id).to_string()
        },
        import_target: if left.tier <= right.tier {
            left.id.clone()
        } else {
            right.id.clone()
        },
        evidence: DuplicateEvidence {
            similarity_metric: "lego-ir-structural-similarity",
            left_metric: format!(
                "tier={:?}:own_nodes={}:composed_nodes={}:fingerprint_bytes={}",
                left.tier,
                left.own_nodes,
                left.composed_nodes,
                left.fingerprint.len()
            ),
            right_metric: format!(
                "tier={:?}:own_nodes={}:composed_nodes={}:fingerprint_bytes={}",
                right.tier,
                right.own_nodes,
                right.composed_nodes,
                right.fingerprint.len()
            ),
            dedup_action: "extract_shared_tier_2_5_primitive_or_compose_existing_op",
        },
    }
}

fn lego_duplicate_subject(op: &OpInfo) -> DuplicateSubject {
    registered_op_duplicate_subject(
        &op.id,
        &op.fingerprint,
        op.own_nodes + op.composed_nodes,
    )
}

#[cfg(test)]
mod check_8_9_10_tests {
    use super::*;

    #[test]
    fn leaf_stem_drops_first_underscore_suffix() {
        assert_eq!(leaf_stem("matmul"), "matmul");
        assert_eq!(leaf_stem("matmul_tiled"), "matmul");
        assert_eq!(leaf_stem("matmul_strassen_one_level"), "matmul");
        assert_eq!(leaf_stem("fft_radix2"), "fft");
        assert_eq!(leaf_stem(""), "");
    }

    #[test]
    fn duplicate_report_json_arg_accepts_path() {
        let args = vec![
            "--with-repo".to_string(),
            "--duplicate-report-json".to_string(),
            "release/evidence/dedup/lego-duplicates.json".to_string(),
        ];
        assert_eq!(
            duplicate_report_json_arg(&args),
            Some(PathBuf::from(
                "release/evidence/dedup/lego-duplicates.json"
            ))
        );
    }
}
