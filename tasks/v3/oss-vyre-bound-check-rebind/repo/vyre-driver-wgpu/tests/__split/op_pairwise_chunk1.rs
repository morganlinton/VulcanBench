// RELEASE PROOF LANE 4  -  pairwise op composition proptest.
//
// Generates random pairwise compositions from the Cat-A operator catalog,
// fuses them via `vyre_foundation::execution_plan::fusion::fuse_programs`,
// and asserts CPU-reference vs GPU-backend parity on the fused Program.
//
// **Proving test** (`pairwise_composition_parity`): only compatible dtype
// signatures are exercised; every passing case proves that sequential
// buffer-wired composition is sound.
//
// **Adversarial test** (`pairwise_composition_adversarial`): unfiltered pairs
// hit `try_compose`.  Incompatible pairs must return `Err`  -  never panic,
// never produce a silent-wrong Program.
//
// Coverage: `vyre_libs::harness::all_entries()`.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use proptest::prelude::*;
use vyre::ir::{BufferAccess, BufferDecl, Expr, Node, Program};
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::execution_plan::fusion::fuse_programs;
use vyre_libs::harness::fp_contract;
use vyre_reference::value::Value;

// ------------------------------------------------------------------
// Catalog
// ------------------------------------------------------------------

/// Unified view of a harness entry so the proptest can treat the whole
/// catalog as a flat vector.
struct UnifiedEntry {
    id: &'static str,
    build: fn() -> Program,
    #[allow(clippy::type_complexity)]
    test_inputs: Option<fn() -> Vec<Vec<Vec<u8>>>>,
}

fn all_entries_vec() -> Vec<UnifiedEntry> {
    let mut out = Vec::new();
    for e in vyre_libs::harness::all_entries() {
        out.push(UnifiedEntry {
            id: e.id,
            build: e.build,
            test_inputs: e.test_inputs,
        });
    }
    out
}

fn entry_count() -> usize {
    all_entries_vec().len()
}

fn entry_by_index(idx: usize) -> &'static UnifiedEntry {
    static ENTRIES: OnceLock<Vec<UnifiedEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(all_entries_vec)
        .get(idx)
        .expect("Fix: entry index out of bounds")
}

// ------------------------------------------------------------------
// GPU backend probe (lazy, fatal when absent)
// ------------------------------------------------------------------

fn gpu() -> &'static WgpuBackend {
    static GPU: OnceLock<WgpuBackend> = OnceLock::new();
    GPU.get_or_init(|| {
        WgpuBackend::acquire().unwrap_or_else(|error| {
            panic!(
                "Fix: pairwise GPU parity could not acquire WGPU backend on a GPU-required host: {error}"
            )
        })
    })
}

fn missing_capability_reason(program: &Program) -> Option<String> {
    let required = vyre_foundation::program_caps::scan(program);
    let backend = gpu();
    vyre_foundation::program_caps::check_backend_capabilities(
        backend.id(),
        backend.supports_subgroup_ops(),
        backend.supports_f16(),
        backend.supports_bf16(),
        backend.supports_indirect_dispatch(),
        true,
        backend.supports_distributed_collectives(),
        backend.max_workgroup_size(),
        &required,
    )
    .err()
    .map(|e| e.to_string())
}

// ------------------------------------------------------------------
// Buffer renaming helpers (composition wiring)
// ------------------------------------------------------------------

fn rename_buffer_in_expr(expr: &Expr, old: &str, new: &str) -> Expr {
    match expr {
        Expr::Load { buffer, index } => Expr::Load {
            buffer: if buffer.as_str() == old {
                new.into()
            } else {
                buffer.clone()
            },
            index: Box::new(rename_buffer_in_expr(index, old, new)),
        },
        Expr::BufLen { buffer } => Expr::BufLen {
            buffer: if buffer.as_str() == old {
                new.into()
            } else {
                buffer.clone()
            },
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op: *op,
            buffer: if buffer.as_str() == old {
                new.into()
            } else {
                buffer.clone()
            },
            index: Box::new(rename_buffer_in_expr(index, old, new)),
            expected: expected
                .as_ref()
                .map(|e| Box::new(rename_buffer_in_expr(e, old, new))),
            value: Box::new(rename_buffer_in_expr(value, old, new)),
            ordering: *ordering,
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op: *op,
            left: Box::new(rename_buffer_in_expr(left, old, new)),
            right: Box::new(rename_buffer_in_expr(right, old, new)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op: op.clone(),
            operand: Box::new(rename_buffer_in_expr(operand, old, new)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id: op_id.clone(),
            args: args
                .iter()
                .map(|a| rename_buffer_in_expr(a, old, new))
                .collect(),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(rename_buffer_in_expr(cond, old, new)),
            true_val: Box::new(rename_buffer_in_expr(true_val, old, new)),
            false_val: Box::new(rename_buffer_in_expr(false_val, old, new)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target: target.clone(),
            value: Box::new(rename_buffer_in_expr(value, old, new)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(rename_buffer_in_expr(a, old, new)),
            b: Box::new(rename_buffer_in_expr(b, old, new)),
            c: Box::new(rename_buffer_in_expr(c, old, new)),
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(rename_buffer_in_expr(cond, old, new)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(rename_buffer_in_expr(value, old, new)),
            lane: Box::new(rename_buffer_in_expr(lane, old, new)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce {
            op: *op,
            value: Box::new(rename_buffer_in_expr(value, old, new)),
        },
        // Leaf expressions  -  no buffers inside.
        _ => expr.clone(),
    }
}

fn rename_buffer_in_node(node: &Node, old: &str, new: &str) -> Node {
    match node {
        Node::Let { name, value } => Node::Let {
            name: name.clone(),
            value: rename_buffer_in_expr(value, old, new),
        },
        Node::Assign { name, value } => Node::Assign {
            name: name.clone(),
            value: rename_buffer_in_expr(value, old, new),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::Store {
            buffer: if buffer.as_str() == old {
                new.into()
            } else {
                buffer.clone()
            },
            index: rename_buffer_in_expr(index, old, new),
            value: rename_buffer_in_expr(value, old, new),
        },
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond: rename_buffer_in_expr(cond, old, new),
            then: then
                .iter()
                .map(|n| rename_buffer_in_node(n, old, new))
                .collect(),
            otherwise: otherwise
                .iter()
                .map(|n| rename_buffer_in_node(n, old, new))
                .collect(),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var: var.clone(),
            from: rename_buffer_in_expr(from, old, new),
            to: rename_buffer_in_expr(to, old, new),
            body: body
                .iter()
                .map(|n| rename_buffer_in_node(n, old, new))
                .collect(),
        },
        Node::Block(nodes) => Node::Block(
            nodes
                .iter()
                .map(|n| rename_buffer_in_node(n, old, new))
                .collect(),
        ),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(
                body.iter()
                    .map(|n| rename_buffer_in_node(n, old, new))
                    .collect(),
            ),
        },
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => Node::IndirectDispatch {
            count_buffer: if count_buffer.as_str() == old {
                new.into()
            } else {
                count_buffer.clone()
            },
            count_offset: *count_offset,
        },
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncLoad {
            source: if source.as_str() == old {
                new.into()
            } else {
                source.clone()
            },
            destination: if destination.as_str() == old {
                new.into()
            } else {
                destination.clone()
            },
            offset: Box::new(rename_buffer_in_expr(offset, old, new)),
            size: Box::new(rename_buffer_in_expr(size, old, new)),
            tag: tag.clone(),
        },
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncStore {
            source: if source.as_str() == old {
                new.into()
            } else {
                source.clone()
            },
            destination: if destination.as_str() == old {
                new.into()
            } else {
                destination.clone()
            },
            offset: Box::new(rename_buffer_in_expr(offset, old, new)),
            size: Box::new(rename_buffer_in_expr(size, old, new)),
            tag: tag.clone(),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(rename_buffer_in_expr(address, old, new)),
            tag: tag.clone(),
        },
        // Catch-all for extension variants this wiring pass does not rewrite.
        _ => node.clone(),
    }
}

fn rename_buffer_in_program(prog: &Program, old: &str, new: &str) -> Program {
    let buffers: Vec<BufferDecl> = prog
        .buffers()
        .iter()
        .map(|buf| {
            let mut b = buf.clone();
            if b.name.as_ref() == old {
                b.name = Arc::from(new);
            }
            b
        })
        .collect();
    let entry: Vec<Node> = prog
        .entry()
        .iter()
        .map(|n| rename_buffer_in_node(n, old, new))
        .collect();
    Program::wrapped(buffers, prog.workgroup_size(), entry)
}

// ------------------------------------------------------------------
// Composition logic
// ------------------------------------------------------------------

/// Attempt to compose `op_a` followed by `op_b` via shared-buffer fusion.
///
/// Returns `Ok(fused_program)` when:
/// * `op_a` has exactly one ReadWrite output buffer,
/// * `op_b` has at least one ReadOnly/Uniform input buffer,
/// * the output/input element types match,
/// * the output/input counts are compatible (both zero, or equal, or one is zero
///   and the test-input byte lengths line up).
///
/// Returns `Err(reason)` for every other pair so the adversarial test can
/// assert clean rejection.
fn try_compose(a: &UnifiedEntry, b: &UnifiedEntry) -> Result<Program, String> {
    let prog_a = (a.build)();
    let prog_b = (b.build)();

    // ---- op_a output analysis ----
    let a_outputs: Vec<&BufferDecl> = prog_a
        .buffers()
        .iter()
        .filter(|buf| buf.access() == BufferAccess::ReadWrite)
        .collect();
    if a_outputs.is_empty() {
        return Err(format!(
            "Fix: {} has no ReadWrite output buffer; cannot wire into downstream op.",
            a.id
        ));
    }
    // Prefer the buffer explicitly marked `is_output`; fall back to first RW.
    let a_out = a_outputs
        .iter()
        .find(|buf| buf.is_output())
        .copied()
        .unwrap_or(a_outputs[0]);

    // ---- op_b input analysis ----
    let b_inputs: Vec<&BufferDecl> = prog_b
        .buffers()
        .iter()
        .filter(|buf| matches!(buf.access(), BufferAccess::ReadOnly | BufferAccess::Uniform))
        .collect();
    if b_inputs.is_empty() {
        return Err(format!(
            "Fix: {} has no ReadOnly/Uniform input buffer; nothing can be wired from the upstream op.",
            b.id
        ));
    }
    let b_in = b_inputs[0];

    // ---- dtype check ----
    if a_out.element() != b_in.element() {
        return Err(format!(
            "Fix: dtype mismatch: {} output={:?} vs {} input={:?}. Add an explicit cast/composition adapter before fusing.",
            a.id,
            a_out.element(),
            b.id,
            b_in.element()
        ));
    }

    // ---- count / shape check ----
    let a_count = a_out.count();
    let b_count = b_in.count();
    if a_count != 0 && b_count != 0 && a_count != b_count {
        return Err(format!(
            "Fix: count mismatch: {} output count={} vs {} input count={}. Add a shape adapter before fusing.",
            a.id, a_count, b.id, b_count
        ));
    }

    // ---- collision check & rename ----
    let a_names: HashSet<&str> = prog_a.buffers().iter().map(|buf| buf.name()).collect();
    let b_names: HashSet<&str> = prog_b.buffers().iter().map(|buf| buf.name()).collect();

    let mut prog_b_prepared = prog_b.clone();

    // Rename every colliding buffer in op_b (except the wired input) so that
    // `fuse_programs` does not accidentally alias unrelated buffers.
    for colliding in b_names.intersection(&a_names) {
        if *colliding == b_in.name() {
            continue;
        }
        let new_name = format!("b_{}", colliding);
        prog_b_prepared = rename_buffer_in_program(&prog_b_prepared, colliding, &new_name);
    }

    // Wire op_b's first input to op_a's output buffer name.
    prog_b_prepared = rename_buffer_in_program(&prog_b_prepared, b_in.name(), a_out.name());

    // ---- fuse ----
    fuse_programs(&[prog_a, prog_b_prepared])
        .map_err(|e| format!("Fix: fusion failed for {} -> {}: {}", a.id, b.id, e))
}
