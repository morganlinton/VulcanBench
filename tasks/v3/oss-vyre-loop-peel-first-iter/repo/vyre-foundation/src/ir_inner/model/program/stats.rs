use super::Program;
use crate::ir::{DataType, Expr, Node};

const CAP_SUBGROUP_OPS: u32 = 1 << 0;
const CAP_F16: u32 = 1 << 1;
const CAP_BF16: u32 = 1 << 2;
const CAP_F64: u32 = 1 << 3;
const CAP_ASYNC_DISPATCH: u32 = 1 << 4;
const CAP_INDIRECT_DISPATCH: u32 = 1 << 5;
const CAP_TENSOR_OPS: u32 = 1 << 6;
const CAP_TRAP: u32 = 1 << 7;
const CAP_DISTRIBUTED_COLLECTIVES: u32 = 1 << 8;

// Bit positions for `ProgramStats::node_kinds_present`. Mirrors the
// variant declaration order in `ir_inner::model::generated::Node` and
// matches `optimizer::program_soa::NodeKind` so the optimizer can use
// either source of truth interchangeably. The
// `node_kinds_present_matches_program_soa_node_kind` test enforces
// the alignment.
/// `Node::Let`.
pub const NODE_KIND_LET: u32 = 1 << 0;
/// `Node::Assign`.
pub const NODE_KIND_ASSIGN: u32 = 1 << 1;
/// `Node::Store`.
pub const NODE_KIND_STORE: u32 = 1 << 2;
/// `Node::If`.
pub const NODE_KIND_IF: u32 = 1 << 3;
/// `Node::Loop`.
pub const NODE_KIND_LOOP: u32 = 1 << 4;
/// `Node::IndirectDispatch`.
pub const NODE_KIND_INDIRECT_DISPATCH: u32 = 1 << 5;
/// `Node::AsyncLoad`.
pub const NODE_KIND_ASYNC_LOAD: u32 = 1 << 6;
/// `Node::AsyncStore`.
pub const NODE_KIND_ASYNC_STORE: u32 = 1 << 7;
/// `Node::AsyncWait`.
pub const NODE_KIND_ASYNC_WAIT: u32 = 1 << 8;
/// `Node::Trap`.
pub const NODE_KIND_TRAP: u32 = 1 << 9;
/// `Node::Resume`.
pub const NODE_KIND_RESUME: u32 = 1 << 10;
/// `Node::Return`.
pub const NODE_KIND_RETURN: u32 = 1 << 11;
/// `Node::Barrier`.
pub const NODE_KIND_BARRIER: u32 = 1 << 12;
/// `Node::Block`.
pub const NODE_KIND_BLOCK: u32 = 1 << 13;
/// `Node::Region`.
pub const NODE_KIND_REGION: u32 = 1 << 14;
/// `Node::Opaque`.
pub const NODE_KIND_ALL_REDUCE: u32 = 1 << 15;
/// `Node::AllGather`.
pub const NODE_KIND_ALL_GATHER: u32 = 1 << 16;
/// `Node::ReduceScatter`.
pub const NODE_KIND_REDUCE_SCATTER: u32 = 1 << 17;
/// `Node::Broadcast`.
pub const NODE_KIND_BROADCAST: u32 = 1 << 18;
/// `Node::Opaque`.
pub const NODE_KIND_OPAQUE: u32 = 1 << 19;

/// Mask covering every node kind that owns an `Expr` tree, i.e. every
/// kind a generic expression-rewriting pass (`canonicalize`, `const_fold`,
/// `strength_reduce`, ...) could possibly affect. A program whose
/// `node_kinds_present` and this mask AND to zero is structurally
/// expression-free and any such pass can SKIP without walking.
pub const NODE_KIND_EXPRESSION_BEARING_MASK: u32 = NODE_KIND_LET
    | NODE_KIND_ASSIGN
    | NODE_KIND_STORE
    | NODE_KIND_IF
    | NODE_KIND_LOOP
    | NODE_KIND_ASYNC_LOAD
    | NODE_KIND_ASYNC_STORE
    | NODE_KIND_TRAP;

/// Aggregated statistics computed from a single walk of a [`Program`].
///
/// This struct is cached inside [`Program`] via a [`std::sync::OnceLock`]
/// so that planning passes (execution plan, capability scan, provenance,
/// fusion) can read constant-time summaries instead of re-walking the IR.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgramStats {
    /// Total statement-node count (includes nested children).
    pub node_count: usize,
    /// Number of `Node::Region` nodes in the full tree.
    pub region_count: u32,
    /// Number of `Expr::Call` expressions.
    pub call_count: u32,
    /// Number of `Node::Opaque` nodes and `Expr::Opaque` expressions.
    pub opaque_count: u32,
    /// Number of top-level `Node::Region` wrappers in `program.entry()`.
    pub top_level_regions: u32,
    /// Sum of statically-known buffer byte sizes.
    pub static_storage_bytes: u64,
    /// Estimated scalar/vector IR instruction count.
    pub instruction_count: u64,
    /// Number of explicit memory operations (loads, stores, async copies).
    pub memory_op_count: u64,
    /// Number of atomic read-modify-write expressions.
    pub atomic_op_count: u64,
    /// Number of control-flow operations.
    pub control_flow_count: u64,
    /// Coarse register pressure estimate from simultaneously named SSA-ish values.
    pub register_pressure_estimate: u32,
    /// Bitmask of capability requirements (see `CAP_*` constants).
    pub capability_bits: u32,
    /// Bitset of every `Node` variant observed during the stats walk
    /// (see the `NODE_KIND_*` constants in this module). Lets pass
    /// `analyze_impl` predicates do an O(1) bit test against the
    /// shared, OnceLock-cached `ProgramStats` instead of recursing
    /// the entry tree just to check 'does this program contain at
    /// least one Loop / If / Atomic / etc.'.
    pub node_kinds_present: u32,
}

mod methods;
impl Program {
    /// Return cached statistics for this program, computing them on first call.
    #[must_use]
    #[inline]
    pub fn stats(&self) -> &ProgramStats {
        self.stats
            .get_or_init(|| std::sync::Arc::new(compute_stats(self)))
            .as_ref()
    }
}

/// Single-pass preorder walk that accumulates every field of [`ProgramStats`].
pub(crate) fn compute_stats(program: &Program) -> ProgramStats {
    let mut node_count = 0usize;
    let mut region_count = 0u32;
    let mut call_count = 0u32;
    let mut opaque_count = 0u32;
    let mut capability_bits = 0u32;
    let mut node_kinds_present = 0u32;
    let mut static_storage_bytes = 0u64;
    let mut ir = IrCounters::default();

    for decl in program.buffers.iter() {
        let count = decl.count();
        if count != 0 {
            if let Some(elem) = decl.element().size_bytes() {
                static_storage_bytes =
                    static_storage_bytes.saturating_add(u64::from(count) * elem as u64);
            }
        }
        mark_datatype_bits(&decl.element(), &mut capability_bits);
    }

    for node in program.entry.iter() {
        walk_node(
            node,
            &mut node_count,
            &mut region_count,
            &mut call_count,
            &mut opaque_count,
            &mut capability_bits,
            &mut node_kinds_present,
            &mut ir,
        );
    }

    let top_level_regions = program
        .entry()
        .iter()
        .filter(|n| matches!(n, Node::Region { .. }))
        .count()
        .try_into()
        .unwrap_or(u32::MAX);

    ProgramStats {
        node_count,
        region_count,
        call_count,
        opaque_count,
        top_level_regions,
        static_storage_bytes,
        instruction_count: ir.instruction_count,
        memory_op_count: ir.memory_op_count,
        atomic_op_count: ir.atomic_op_count,
        control_flow_count: ir.control_flow_count,
        register_pressure_estimate: ir.register_pressure_estimate(),
        capability_bits,
        node_kinds_present,
    }
}

#[derive(Default)]
struct IrCounters {
    instruction_count: u64,
    memory_op_count: u64,
    atomic_op_count: u64,
    control_flow_count: u64,
    live_names: u32,
    max_live_names: u32,
}

impl IrCounters {
    fn instruction(&mut self) {
        self.instruction_count = self.instruction_count.saturating_add(1);
    }

    fn memory(&mut self) {
        self.memory_op_count = self.memory_op_count.saturating_add(1);
        self.instruction();
    }

    fn atomic(&mut self) {
        self.atomic_op_count = self.atomic_op_count.saturating_add(1);
        self.memory();
    }

    fn control_flow(&mut self) {
        self.control_flow_count = self.control_flow_count.saturating_add(1);
        self.instruction();
    }

    fn bind_name(&mut self) {
        self.live_names = self.live_names.saturating_add(1);
        self.max_live_names = self.max_live_names.max(self.live_names);
    }

    fn enter_scope(&mut self) -> u32 {
        self.live_names
    }

    fn leave_scope(&mut self, saved: u32) {
        self.live_names = saved;
    }

    fn register_pressure_estimate(&self) -> u32 {
        self.max_live_names
    }
}

#[inline]
fn mark_datatype_bits(ty: &DataType, bits: &mut u32) {
    match ty {
        DataType::F16 => *bits |= CAP_F16,
        DataType::BF16 => *bits |= CAP_BF16,
        DataType::F64 => *bits |= CAP_F64,
        DataType::Tensor | DataType::TensorShaped { .. } => *bits |= CAP_TENSOR_OPS,
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::too_many_lines,
    reason = "single-pass ProgramStats walker keeps all counters hot and avoids repeated IR traversals"
)]
fn walk_node(
    node: &Node,
    nodes: &mut usize,
    regions: &mut u32,
    calls: &mut u32,
    opaque: &mut u32,
    bits: &mut u32,
    kinds: &mut u32,
    ir: &mut IrCounters,
) {
    *nodes = nodes.saturating_add(1);
    match node {
        Node::Let { value, .. } => {
            *kinds |= NODE_KIND_LET;
            ir.instruction();
            ir.bind_name();
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::Assign { value, .. } => {
            *kinds |= NODE_KIND_ASSIGN;
            ir.instruction();
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::Store { index, value, .. } => {
            *kinds |= NODE_KIND_STORE;
            ir.memory();
            walk_expr(index, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            *kinds |= NODE_KIND_IF;
            ir.control_flow();
            walk_expr(cond, nodes, regions, calls, opaque, bits, kinds, ir);
            let saved = ir.enter_scope();
            for child in then.iter().chain(otherwise.iter()) {
                walk_node(child, nodes, regions, calls, opaque, bits, kinds, ir);
            }
            ir.leave_scope(saved);
        }
        Node::Loop { from, to, body, .. } => {
            *kinds |= NODE_KIND_LOOP;
            ir.control_flow();
            walk_expr(from, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(to, nodes, regions, calls, opaque, bits, kinds, ir);
            let saved = ir.enter_scope();
            for child in body {
                walk_node(child, nodes, regions, calls, opaque, bits, kinds, ir);
            }
            ir.leave_scope(saved);
        }
        Node::Block(children) => {
            *kinds |= NODE_KIND_BLOCK;
            let saved = ir.enter_scope();
            for child in children {
                walk_node(child, nodes, regions, calls, opaque, bits, kinds, ir);
            }
            ir.leave_scope(saved);
        }
        Node::Region { body, .. } => {
            *kinds |= NODE_KIND_REGION;
            *regions = regions.saturating_add(1);
            let saved = ir.enter_scope();
            for child in body.iter() {
                walk_node(child, nodes, regions, calls, opaque, bits, kinds, ir);
            }
            ir.leave_scope(saved);
        }
        Node::AsyncLoad { offset, size, .. } => {
            *kinds |= NODE_KIND_ASYNC_LOAD;
            *bits |= CAP_ASYNC_DISPATCH;
            ir.memory();
            walk_expr(offset, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(size, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::AsyncStore { offset, size, .. } => {
            *kinds |= NODE_KIND_ASYNC_STORE;
            *bits |= CAP_ASYNC_DISPATCH;
            ir.memory();
            walk_expr(offset, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(size, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::AsyncWait { .. } => {
            *kinds |= NODE_KIND_ASYNC_WAIT;
            *bits |= CAP_ASYNC_DISPATCH;
            ir.control_flow();
        }
        Node::IndirectDispatch { .. } => {
            *kinds |= NODE_KIND_INDIRECT_DISPATCH;
            *bits |= CAP_INDIRECT_DISPATCH;
            ir.control_flow();
        }
        Node::Trap { address, .. } => {
            *kinds |= NODE_KIND_TRAP;
            *bits |= CAP_TRAP;
            ir.control_flow();
            walk_expr(address, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Node::AllReduce { .. } => {
            *kinds |= NODE_KIND_ALL_REDUCE;
            *bits |= CAP_DISTRIBUTED_COLLECTIVES;
            ir.memory();
        }
        Node::AllGather { .. } => {
            *kinds |= NODE_KIND_ALL_GATHER;
            *bits |= CAP_DISTRIBUTED_COLLECTIVES;
            ir.memory();
        }
        Node::ReduceScatter { .. } => {
            *kinds |= NODE_KIND_REDUCE_SCATTER;
            *bits |= CAP_DISTRIBUTED_COLLECTIVES;
            ir.memory();
        }
        Node::Broadcast { .. } => {
            *kinds |= NODE_KIND_BROADCAST;
            *bits |= CAP_DISTRIBUTED_COLLECTIVES;
            ir.memory();
        }
        Node::Opaque(_) => {
            *kinds |= NODE_KIND_OPAQUE;
            *opaque = opaque.saturating_add(1);
            ir.instruction();
        }
        Node::Return => {
            *kinds |= NODE_KIND_RETURN;
            ir.control_flow();
        }
        Node::Barrier { .. } => {
            *kinds |= NODE_KIND_BARRIER;
            ir.control_flow();
        }
        Node::Resume { .. } => {
            *kinds |= NODE_KIND_RESUME;
            ir.control_flow();
        }
    }
}

#[allow(clippy::only_used_in_recursion, clippy::too_many_arguments)]
fn walk_expr(
    expr: &Expr,
    nodes: &mut usize,
    regions: &mut u32,
    calls: &mut u32,
    opaque: &mut u32,
    bits: &mut u32,
    kinds: &mut u32,
    ir: &mut IrCounters,
) {
    match expr {
        Expr::SubgroupReduce { value, .. } => {
            *bits |= CAP_SUBGROUP_OPS;
            ir.instruction();
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::SubgroupBallot { cond } => {
            *bits |= CAP_SUBGROUP_OPS;
            ir.instruction();
            walk_expr(cond, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::SubgroupShuffle { value, lane } => {
            *bits |= CAP_SUBGROUP_OPS;
            ir.instruction();
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(lane, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::BinOp { left, right, .. } => {
            ir.instruction();
            walk_expr(left, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(right, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::UnOp { operand, .. } => {
            ir.instruction();
            walk_expr(operand, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Fma { a, b, c } => {
            ir.instruction();
            walk_expr(a, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(b, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(c, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            ir.instruction();
            walk_expr(cond, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(true_val, nodes, regions, calls, opaque, bits, kinds, ir);
            walk_expr(false_val, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Cast { target, value } => {
            mark_datatype_bits(target, bits);
            ir.instruction();
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Load { index, .. } => {
            ir.memory();
            walk_expr(index, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Call { op_id, args } => {
            if is_subgroup_intrinsic_id(op_id) {
                *bits |= CAP_SUBGROUP_OPS;
            }
            *calls = calls.saturating_add(1);
            ir.instruction();
            for arg in args {
                walk_expr(arg, nodes, regions, calls, opaque, bits, kinds, ir);
            }
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            ir.atomic();
            walk_expr(index, nodes, regions, calls, opaque, bits, kinds, ir);
            if let Some(expected) = expected.as_deref() {
                walk_expr(expected, nodes, regions, calls, opaque, bits, kinds, ir);
            }
            walk_expr(value, nodes, regions, calls, opaque, bits, kinds, ir);
        }
        Expr::Opaque(_) => {
            *opaque = opaque.saturating_add(1);
            ir.instruction();
        }
        Expr::SubgroupLocalId | Expr::SubgroupSize => {
            *bits |= CAP_SUBGROUP_OPS;
            ir.instruction();
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => {}
    }
}

fn is_subgroup_intrinsic_id(op_id: &str) -> bool {
    const MARKERS: &[&str] = &[
        "subgroup_",
        "::subgroup::",
        "::subgroup",
        "wave_",
        "::wave::",
        "warp_",
        "::warp::",
    ];
    MARKERS.iter().any(|marker| op_id.contains(marker))
}
