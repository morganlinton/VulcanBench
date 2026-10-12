// Composition discipline CI gates.
//
// These tests enforce the "After Effects" compositional architecture:
//
// 1. **No monoliths**  -  every registered op must stay under a complexity
//    budget. If it exceeds the threshold, the author must split the op
//    into smaller, reusable compositions.
//
// 2. **No reimplementation**  -  if an op's IR contains a subgraph that
//    structurally matches another registered op, the author must call
//    that op via `Expr::Call` instead of inlining its logic.
//
// Together these gates enforce a composition ratchet: the op catalog
// grows organically, and every new composition automatically benefits
// every pipeline that calls it.
//
// This module-level doc lives as `//` line comments rather than `//!`
// inner doc comments because the file is `include!()`-d into the
// parent test crate; an inner doc on an `include!`-d chunk attaches
// to the enclosing module and conflicts with chunk-2.

use vyre::ir::{Expr, Node, Program};

// ───────────────────────────────────────────────────────────────────
// Complexity measurement
// ───────────────────────────────────────────────────────────────────

/// Complexity stats for a single registered op.
#[derive(Debug, Clone, Copy)]
struct Complexity {
    /// Total number of IR statement nodes (recursive).
    total_nodes: usize,
    /// Total number of IR expression nodes (recursive).
    total_exprs: usize,
    /// Maximum nesting depth of control-flow nodes (If / Loop).
    max_depth: usize,
    /// Number of Loop nodes.
    loop_count: usize,
}

fn measure_program(program: &Program) -> Complexity {
    let mut stats = Complexity {
        total_nodes: 0,
        total_exprs: 0,
        max_depth: 0,
        loop_count: 0,
    };
    for node in program.entry() {
        measure_node(node, 0, &mut stats);
    }
    stats
}

fn measure_node(node: &Node, depth: usize, stats: &mut Complexity) {
    stats.total_nodes += 1;
    stats.max_depth = stats.max_depth.max(depth);
    match node {
        Node::Let { value, .. } => {
            count_expr(value, stats);
        }
        Node::Assign { value, .. } => {
            count_expr(value, stats);
        }
        Node::Store { index, value, .. } => {
            count_expr(index, stats);
            count_expr(value, stats);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            count_expr(cond, stats);
            for n in then {
                measure_node(n, depth + 1, stats);
            }
            for n in otherwise {
                measure_node(n, depth + 1, stats);
            }
        }
        Node::Loop { from, to, body, .. } => {
            stats.loop_count += 1;
            count_expr(from, stats);
            count_expr(to, stats);
            for n in body {
                measure_node(n, depth + 1, stats);
            }
        }
        Node::Block(nodes) => {
            for n in nodes {
                measure_node(n, depth, stats);
            }
        }
        Node::Region {
            source_region,
            body,
            ..
        } => {
            if is_child_composition(source_region.as_ref().map(|r| r.name.as_str())) {
                return;
            }
            for n in body.iter() {
                measure_node(n, depth, stats);
            }
        }
        Node::Return
        | Node::Barrier {
            ordering: vyre::memory_model::MemoryOrdering::SeqCst,
        }
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncWait { .. }
        | Node::Opaque(_) => {}
        // Non-exhaustive variants are leaf nodes for this structural budget.
        _ => {}
    }
}

fn count_expr(expr: &Expr, stats: &mut Complexity) {
    stats.total_exprs += 1;
    match expr {
        Expr::BinOp { left, right, .. } => {
            count_expr(left, stats);
            count_expr(right, stats);
        }
        Expr::UnOp { operand, .. } => {
            count_expr(operand, stats);
        }
        Expr::Load { index, .. } => {
            count_expr(index, stats);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            count_expr(cond, stats);
            count_expr(true_val, stats);
            count_expr(false_val, stats);
        }
        Expr::Cast { value, .. } => {
            count_expr(value, stats);
        }
        Expr::Fma { a, b, c } => {
            count_expr(a, stats);
            count_expr(b, stats);
            count_expr(c, stats);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            count_expr(index, stats);
            if let Some(exp) = expected {
                count_expr(exp, stats);
            }
            count_expr(value, stats);
        }
        Expr::SubgroupBallot { cond } => {
            count_expr(cond, stats);
        }
        Expr::SubgroupShuffle { value, lane } => {
            count_expr(value, stats);
            count_expr(lane, stats);
        }
        Expr::SubgroupReduce { value, .. } => {
            count_expr(value, stats);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                count_expr(arg, stats);
            }
        }
        // Leaf expressions
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::Opaque(_) => {}
        // Non-exhaustive variants are leaf exprs for this structural budget.
        _ => {}
    }
}

// ───────────────────────────────────────────────────────────────────
// Structural fingerprinting for subsumption detection
// ───────────────────────────────────────────────────────────────────

/// Hash the structural "shape" of a program's entry nodes, ignoring local
/// binding names while preserving literal values, buffer roles, op targets,
/// and child-composition bodies. Two ops with isomorphic control flow and
/// identical semantic constants produce the same fingerprint.
fn structural_fingerprint(program: &Program) -> u64 {
    let mut hasher = 0u64;
    for node in program.entry() {
        hash_node(node, &mut hasher);
    }
    hasher
}

fn hash_node(node: &Node, h: &mut u64) {
    match node {
        Node::Let { value, .. } => {
            mix(h, 1);
            hash_expr(value, h);
        }
        Node::Assign { value, .. } => {
            mix(h, 2);
            hash_expr(value, h);
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            mix(h, 3);
            hash_str(buffer.as_str(), h);
            hash_expr(index, h);
            hash_expr(value, h);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            mix(h, 4);
            hash_expr(cond, h);
            mix(h, then.len() as u64);
            for n in then {
                hash_node(n, h);
            }
            mix(h, otherwise.len() as u64);
            for n in otherwise {
                hash_node(n, h);
            }
        }
        Node::Loop { from, to, body, .. } => {
            mix(h, 5);
            hash_expr(from, h);
            hash_expr(to, h);
            mix(h, body.len() as u64);
            for n in body {
                hash_node(n, h);
            }
        }
        Node::Block(nodes) => {
            mix(h, 6);
            for n in nodes {
                hash_node(n, h);
            }
        }
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            mix(h, 7);
            if is_child_composition(source_region.as_ref().map(|r| r.name.as_str())) {
                hash_str(generator.as_str(), h);
            }
            for n in body.iter() {
                hash_node(n, h);
            }
        }
        Node::Return => mix(h, 10),
        Node::Barrier {
            ordering: vyre::memory_model::MemoryOrdering::SeqCst,
        } => mix(h, 11),
        Node::IndirectDispatch { .. } => mix(h, 12),
        Node::AsyncLoad { .. } => mix(h, 13),
        Node::AsyncWait { .. } => mix(h, 14),
        Node::Opaque(_) => mix(h, 15),
        _ => mix(h, 16),
    }
}

fn hash_expr(expr: &Expr, h: &mut u64) {
    match expr {
        Expr::LitU32(value) => {
            mix(h, 100);
            mix(h, u64::from(*value));
        }
        Expr::LitI32(value) => {
            mix(h, 101);
            mix(h, *value as u32 as u64);
        }
        Expr::LitF32(value) => {
            mix(h, 102);
            mix(h, u64::from(value.to_bits()));
        }
        Expr::LitBool(value) => {
            mix(h, 103);
            mix(h, u64::from(*value));
        }
        Expr::Var(_) => mix(h, 104),
        Expr::Load { buffer, index } => {
            mix(h, 105);
            for byte in buffer.as_str().bytes() {
                mix(h, byte as u64);
            }
            hash_expr(index, h);
        }
        Expr::BufLen { buffer } => {
            mix(h, 106);
            for byte in buffer.as_str().bytes() {
                mix(h, byte as u64);
            }
        }
        Expr::InvocationId { .. } => mix(h, 107),
        Expr::WorkgroupId { .. } => mix(h, 108),
        Expr::LocalId { .. } => mix(h, 109),
        Expr::BinOp { op, left, right } => {
            mix(h, 110);
            // Hash the full operator name to distinguish Add from Mul etc.
            for byte in format!("{op:?}").bytes() {
                mix(h, byte as u64);
            }
            hash_expr(left, h);
            hash_expr(right, h);
        }
        Expr::UnOp { op, operand } => {
            mix(h, 111);
            for byte in format!("{op:?}").bytes() {
                mix(h, byte as u64);
            }
            hash_expr(operand, h);
        }
        Expr::Call { op_id, args } => {
            // CRITIQUE_CONFORM_2026-04-23 H7: hashing only the
            // discriminant + arity collapsed every `Expr::Call` with
            // the same arg count into a single fingerprint. An
            // attacker could trivially craft a call to op `b` whose
            // structural hash matched a call to op `a`, bypassing the
            // cross-namespace subsumption gate. Recurse into every
            // arg and mix the op_id bytes so distinct calls produce
            // distinct fingerprints.
            mix(h, 112);
            for b in op_id.as_bytes() {
                mix(h, u64::from(*b));
            }
            mix(h, args.len() as u64);
            for arg in args {
                hash_expr(arg, h);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            mix(h, 113);
            hash_expr(cond, h);
            hash_expr(true_val, h);
            hash_expr(false_val, h);
        }
        Expr::Cast { target, value } => {
            mix(h, 114);
            for byte in format!("{target:?}").bytes() {
                mix(h, byte as u64);
            }
            hash_expr(value, h);
        }
        Expr::Fma { a, b, c } => {
            mix(h, 115);
            hash_expr(a, h);
            hash_expr(b, h);
            hash_expr(c, h);
        }
        Expr::Atomic {
            op,
            index,
            expected,
            value,
            ..
        } => {
            mix(h, 116);
            for byte in format!("{op:?}").bytes() {
                mix(h, byte as u64);
            }
            hash_expr(index, h);
            if let Some(exp) = expected {
                hash_expr(exp, h);
            }
            hash_expr(value, h);
        }
        Expr::SubgroupBallot { cond } => {
            mix(h, 117);
            hash_expr(cond, h);
        }
        Expr::SubgroupShuffle { value, lane } => {
            mix(h, 118);
            hash_expr(value, h);
            hash_expr(lane, h);
        }
        Expr::SubgroupReduce { value, .. } => {
            mix(h, 119);
            hash_expr(value, h);
        }
        Expr::Opaque(_) => mix(h, 199),
        // Future-proof: unknown variants get a unique tag.
        _ => mix(h, 200),
    }
}

fn is_child_composition(source_region: Option<&str>) -> bool {
    source_region.is_some()
}

fn hash_str(value: &str, h: &mut u64) {
    for byte in value.as_bytes() {
        mix(h, u64::from(*byte));
    }
}

/// FNV-1a–style mixer.
fn mix(h: &mut u64, v: u64) {
    *h ^= v;
    *h = h.wrapping_mul(0x100000001b3);
}

