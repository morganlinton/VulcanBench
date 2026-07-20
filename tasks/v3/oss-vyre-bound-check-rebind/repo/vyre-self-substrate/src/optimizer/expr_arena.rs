//! Expr arena encoded as flat GPU buffers.
//!
//! Every `Expr` in the user's `Program` is assigned a stable `ExprId`
//! and emitted as a 4-tuple `(kind, arg0, arg1, arg2, arg3)` in
//! post-order  -  children before parents  -  so a single linear scan
//! computes anything bottom-up. The kind tags are stable across runs
//! and follow the reservations in `vyre-spec`.
//!
//! This is the keystone for every Expr-level pass:
//! - Const-fold scans post-order, marking foldable Exprs and computing
//!   their literal values.
//! - Canonicalize swaps commutative BinOp operands by structural-hash
//!   ordering.
//! - CSE intern the encoded form to find structurally-equal Exprs.
//! - Pattern-match runs subgroup NFA over the kind sequence.
//!
//! V1 scope: literals, Var, Load, BufLen, Invocation/Workgroup/LocalId,
//! BinOp, UnOp, Select, Fma, SubgroupLocalId, SubgroupSize. Variadic /
//! extension Exprs (Call, Atomic, SubgroupBallot/Shuffle/Add, Cast,
//! Opaque) bail with `EncodeError::Unsupported` until later passes
//! need them  -  adding them is a small extension of the `encode_expr`
//! match arm and a new `expr_kind` constant.

use vyre_foundation::ir::model::types::{BinOp, UnOp};
use vyre_foundation::ir::{Expr, Ident, Node, Program};

use super::encode::EncodeError;

/// Stable Expr kind tags. Values are reserved here; they do not (yet)
/// match a frozen wire format outside this crate, but follow the
/// numbering in `vyre-spec/src/{bin_op,un_op}.rs` for op-tag families
/// so the GPU side can share one truth table.
pub mod expr_kind {
    pub const LIT_U32: u32 = 0x01;
    pub const LIT_I32: u32 = 0x02;
    pub const LIT_F32: u32 = 0x03;
    pub const LIT_BOOL: u32 = 0x04;
    pub const VAR: u32 = 0x05;
    pub const LOAD: u32 = 0x06;
    pub const BUF_LEN: u32 = 0x07;
    pub const INVOCATION_ID: u32 = 0x08;
    pub const WORKGROUP_ID: u32 = 0x09;
    pub const LOCAL_ID: u32 = 0x0A;
    pub const BIN_OP: u32 = 0x0B;
    pub const UN_OP: u32 = 0x0C;
    pub const SELECT: u32 = 0x0D;
    pub const FMA: u32 = 0x0E;
    pub const SUBGROUP_LOCAL_ID: u32 = 0x0F;
    pub const SUBGROUP_SIZE: u32 = 0x10;
}

/// Encoded Expr arena. One row per Expr; row layout depends on kind.
#[derive(Debug, Clone, Default)]
pub struct ExprArenaEncoding {
    /// Total Expr instances encoded.
    pub expr_count: u32,
    /// Per-Expr kind tag (one of `expr_kind::*`).
    pub kinds: Vec<u32>,
    /// First arg slot  -  see kind table in module docs.
    pub arg0: Vec<u32>,
    /// Second arg slot.
    pub arg1: Vec<u32>,
    /// Third arg slot.
    pub arg2: Vec<u32>,
    /// Fourth arg slot (reserved; populated by future kinds with arity ≥ 4).
    pub arg3: Vec<u32>,
    /// Post-order traversal: each ExprId appears AFTER its children.
    /// Today this is just `0..expr_count` because the encoder emits
    /// in post-order natively, but we keep the field explicit so
    /// future passes can rely on it without re-deriving.
    pub post_order: Vec<u32>,
    /// For each Node visited (in DFS prefix order, matching the Node
    /// graph encoder), the ExprIds of every top-level Expr the Node
    /// owns, in canonical slot order. Index `0` is the synthetic ROOT
    /// graph node and has no Exprs.
    pub node_top_level_exprs: Vec<Vec<u32>>,
    /// Per-Expr depth in the post-order arena. Leaves (literals,
    /// vars, builtins, no-child Exprs) have depth 0; parent Exprs have
    /// depth = max(children's depths) + 1. Level-parallel passes
    /// (e.g. const-fold) dispatch one level at a time using this
    /// column.
    pub depths: Vec<u32>,
    /// Maximum value in `depths` (or 0 for an empty arena). The
    /// orchestrator dispatches `max_depth + 1` levels in sequence.
    pub max_depth: u32,
}

/// Encode every `Expr` in `program`'s entry tree (including nested
/// scope bodies) as a flat ExprArenaEncoding. The traversal order
/// matches the Node graph encoder  -  DFS prefix over Nodes, each
/// Node's Exprs walked in canonical slot order, every Expr's children
/// walked first. This guarantees the Node graph's graph-id index and
/// the arena's `node_top_level_exprs` index align one-to-one.
pub fn encode_expr_arena(program: &Program) -> Result<ExprArenaEncoding, EncodeError> {
    let body: &[Node] = match program.entry() {
        [Node::Region { body, .. }] => body.as_slice(),
        entry => entry,
    };

    let mut ctx = ArenaCtx::default();
    // Index 0 mirrors the synthetic ROOT graph node (no Exprs).
    ctx.node_top_level_exprs.push(Vec::new());
    ctx.encode_scope(body)?;
    ctx.post_order = (0..ctx.expr_count).collect();
    let depths = compute_depths(&ctx);
    let max_depth = depths.iter().copied().max().unwrap_or(0);
    Ok(ExprArenaEncoding {
        expr_count: ctx.expr_count,
        kinds: ctx.kinds,
        arg0: ctx.arg0,
        arg1: ctx.arg1,
        arg2: ctx.arg2,
        arg3: ctx.arg3,
        post_order: ctx.post_order,
        node_top_level_exprs: ctx.node_top_level_exprs,
        depths,
        max_depth,
    })
}

/// Compute per-Expr depth in post-order. Leaves (no Expr children)
/// have depth 0; every other kind takes max over its child
/// dependencies + 1.
fn compute_depths(ctx: &ArenaCtx) -> Vec<u32> {
    let mut depths = vec![0u32; ctx.expr_count as usize];
    for i in 0..ctx.expr_count as usize {
        let kind = ctx.kinds[i];
        let depth = match kind {
            // Leaves with no child Expr dependencies.
            expr_kind::LIT_U32
            | expr_kind::LIT_I32
            | expr_kind::LIT_F32
            | expr_kind::LIT_BOOL
            | expr_kind::VAR
            | expr_kind::BUF_LEN
            | expr_kind::INVOCATION_ID
            | expr_kind::WORKGROUP_ID
            | expr_kind::LOCAL_ID
            | expr_kind::SUBGROUP_LOCAL_ID
            | expr_kind::SUBGROUP_SIZE => 0,
            // One child in arg1.
            expr_kind::LOAD | expr_kind::UN_OP => depths[ctx.arg1[i] as usize] + 1,
            // Two children in arg1, arg2.
            expr_kind::BIN_OP => depths[ctx.arg1[i] as usize].max(depths[ctx.arg2[i] as usize]) + 1,
            // Three children in arg0, arg1, arg2.
            expr_kind::SELECT | expr_kind::FMA => {
                depths[ctx.arg0[i] as usize]
                    .max(depths[ctx.arg1[i] as usize])
                    .max(depths[ctx.arg2[i] as usize])
                    + 1
            }
            _ => 0,
        };
        depths[i] = depth;
    }
    depths
}

#[derive(Default)]
struct ArenaCtx {
    expr_count: u32,
    kinds: Vec<u32>,
    arg0: Vec<u32>,
    arg1: Vec<u32>,
    arg2: Vec<u32>,
    arg3: Vec<u32>,
    post_order: Vec<u32>,
    node_top_level_exprs: Vec<Vec<u32>>,
}

impl ArenaCtx {
    fn alloc(&mut self, kind: u32, a0: u32, a1: u32, a2: u32, a3: u32) -> u32 {
        let id = self.expr_count;
        self.expr_count += 1;
        self.kinds.push(kind);
        self.arg0.push(a0);
        self.arg1.push(a1);
        self.arg2.push(a2);
        self.arg3.push(a3);
        id
    }

    fn encode_scope(&mut self, body: &[Node]) -> Result<(), EncodeError> {
        let prefix_len = super::encode::reachable_prefix_len(body);
        for node in &body[..prefix_len] {
            self.encode_node(node)?;
        }
        Ok(())
    }

    fn encode_node(&mut self, node: &Node) -> Result<(), EncodeError> {
        // Allocate this Node's top-level-Expr slot first (matching the
        // Node graph's graph-id allocation order).
        let node_index = self.node_top_level_exprs.len();
        self.node_top_level_exprs.push(Vec::new());

        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                let id = self.encode_expr(value)?;
                self.node_top_level_exprs[node_index].push(id);
            }
            Node::Store { index, value, .. } => {
                let id_index = self.encode_expr(index)?;
                let id_value = self.encode_expr(value)?;
                self.node_top_level_exprs[node_index].push(id_index);
                self.node_top_level_exprs[node_index].push(id_value);
            }
            Node::If { cond, .. } => {
                let id = self.encode_expr(cond)?;
                self.node_top_level_exprs[node_index].push(id);
            }
            Node::Loop { from, to, .. } => {
                let id_from = self.encode_expr(from)?;
                let id_to = self.encode_expr(to)?;
                self.node_top_level_exprs[node_index].push(id_from);
                self.node_top_level_exprs[node_index].push(id_to);
            }
            Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                let id_off = self.encode_expr(offset)?;
                let id_sz = self.encode_expr(size)?;
                self.node_top_level_exprs[node_index].push(id_off);
                self.node_top_level_exprs[node_index].push(id_sz);
            }
            Node::Trap { address, .. } => {
                let id = self.encode_expr(address)?;
                self.node_top_level_exprs[node_index].push(id);
            }
            Node::Return
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::AsyncWait { .. }
            | Node::Resume { .. }
            | Node::Opaque(_)
            | Node::Block(_)
            | Node::Region { .. } => {
                // Wrappers carry no Exprs at this level; their nested
                // bodies are walked by the recursion below.
            }
            _ => {
                return Err(EncodeError::Unsupported(
                    "Fix: ExprArena encoder hit an unknown Node variant; \
                     extend `expr_arena.rs::encode_node`.",
                ));
            }
        }

        // Recurse into nested scope bodies. Each pushes its own
        // node_top_level_exprs entries.
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                self.encode_scope(then)?;
                self.encode_scope(otherwise)?;
            }
            Node::Loop { body, .. } => {
                self.encode_scope(body)?;
            }
            Node::Block(body) => {
                self.encode_scope(body)?;
            }
            Node::Region { body, .. } => {
                self.encode_scope(body.as_slice())?;
            }
            _ => {}
        }
        Ok(())
    }

    fn encode_expr(&mut self, expr: &Expr) -> Result<u32, EncodeError> {
        // Recurse into children FIRST so they get smaller ExprIds  -
        // post-order numbering.
        match expr {
            Expr::LitU32(v) => Ok(self.alloc(expr_kind::LIT_U32, *v, 0, 0, 0)),
            Expr::LitI32(v) => Ok(self.alloc(expr_kind::LIT_I32, *v as u32, 0, 0, 0)),
            Expr::LitF32(v) => Ok(self.alloc(expr_kind::LIT_F32, v.to_bits(), 0, 0, 0)),
            Expr::LitBool(v) => Ok(self.alloc(expr_kind::LIT_BOOL, u32::from(*v), 0, 0, 0)),
            Expr::Var(name) => Ok(self.alloc(expr_kind::VAR, ident_tag(name), 0, 0, 0)),
            Expr::Load { buffer, index } => {
                let idx_id = self.encode_expr(index)?;
                Ok(self.alloc(expr_kind::LOAD, ident_tag(buffer), idx_id, 0, 0))
            }
            Expr::BufLen { buffer } => {
                Ok(self.alloc(expr_kind::BUF_LEN, ident_tag(buffer), 0, 0, 0))
            }
            Expr::InvocationId { axis } => {
                Ok(self.alloc(expr_kind::INVOCATION_ID, u32::from(*axis), 0, 0, 0))
            }
            Expr::WorkgroupId { axis } => {
                Ok(self.alloc(expr_kind::WORKGROUP_ID, u32::from(*axis), 0, 0, 0))
            }
            Expr::LocalId { axis } => {
                Ok(self.alloc(expr_kind::LOCAL_ID, u32::from(*axis), 0, 0, 0))
            }
            Expr::BinOp { op, left, right } => {
                let lid = self.encode_expr(left)?;
                let rid = self.encode_expr(right)?;
                let tag = bin_op_tag(op)?;
                Ok(self.alloc(expr_kind::BIN_OP, tag, lid, rid, 0))
            }
            Expr::UnOp { op, operand } => {
                let cid = self.encode_expr(operand)?;
                let tag = un_op_tag(op)?;
                Ok(self.alloc(expr_kind::UN_OP, tag, cid, 0, 0))
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                let cid = self.encode_expr(cond)?;
                let tid = self.encode_expr(true_val)?;
                let fid = self.encode_expr(false_val)?;
                Ok(self.alloc(expr_kind::SELECT, cid, tid, fid, 0))
            }
            Expr::Fma { a, b, c } => {
                let aid = self.encode_expr(a)?;
                let bid = self.encode_expr(b)?;
                let cid = self.encode_expr(c)?;
                Ok(self.alloc(expr_kind::FMA, aid, bid, cid, 0))
            }
            Expr::SubgroupLocalId => Ok(self.alloc(expr_kind::SUBGROUP_LOCAL_ID, 0, 0, 0, 0)),
            Expr::SubgroupSize => Ok(self.alloc(expr_kind::SUBGROUP_SIZE, 0, 0, 0, 0)),
            // Variadic / extension / typed-payload Exprs not yet
            // supported by the V1 encoder. Adding them is a small
            // extension: pick a kind tag, encode payload across the
            // arg slots, register a decoder.
            Expr::Call { .. } => Err(EncodeError::Unsupported(
                "ExprArena V1: Call (variadic) not yet encoded",
            )),
            Expr::Cast { .. } => Err(EncodeError::Unsupported(
                "ExprArena V1: Cast (DataType payload) not yet encoded",
            )),
            Expr::Atomic { .. } => Err(EncodeError::Unsupported(
                "ExprArena V1: Atomic (multi-payload) not yet encoded",
            )),
            Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. } => Err(EncodeError::Unsupported(
                "ExprArena V1: Subgroup{Ballot,Shuffle,Add} not yet encoded",
            )),
            Expr::Opaque(_) => Err(EncodeError::Unsupported(
                "ExprArena V1: Opaque extension Exprs not yet encoded",
            )),
            _ => Err(EncodeError::Unsupported(
                "Fix: ExprArena encoder hit an unknown Expr variant; \
                 extend `expr_arena.rs::encode_expr`.",
            )),
        }
    }
}

/// Truncate an `Ident::cached_hash` to u32 for the arena ident slot.
/// Hash collisions across different names are possible but extremely
/// rare at typical Program scale; the arena passes that need exact
/// equality should compare two ExprIds directly (they share kind +
/// args byte-for-byte iff structurally identical).
fn ident_tag(ident: &Ident) -> u32 {
    (ident.cached_hash() as u32) ^ ((ident.cached_hash() >> 32) as u32)
}

/// Map a `BinOp` to its frozen u32 tag (per `vyre-spec/src/bin_op.rs`
/// reservations).
fn bin_op_tag(op: &BinOp) -> Result<u32, EncodeError> {
    Ok(match op {
        BinOp::Add => 0x01,
        BinOp::Sub => 0x02,
        BinOp::Mul => 0x03,
        BinOp::Div => 0x04,
        BinOp::Mod => 0x05,
        BinOp::BitAnd => 0x06,
        BinOp::BitOr => 0x07,
        BinOp::BitXor => 0x08,
        BinOp::Shl => 0x09,
        BinOp::Shr => 0x0A,
        BinOp::Eq => 0x0B,
        BinOp::Ne => 0x0C,
        BinOp::Lt => 0x0D,
        BinOp::Gt => 0x0E,
        BinOp::Le => 0x10,
        BinOp::Ge => 0x11,
        BinOp::And => 0x12,
        BinOp::Or => 0x13,
        BinOp::AbsDiff => 0x14,
        BinOp::Min => 0x15,
        BinOp::Max => 0x16,
        BinOp::SaturatingAdd => 0x17,
        BinOp::SaturatingSub => 0x18,
        BinOp::SaturatingMul => 0x19,
        BinOp::Shuffle => 0x1A,
        BinOp::Ballot => 0x1B,
        BinOp::WaveReduce => 0x1C,
        BinOp::WaveBroadcast => 0x1D,
        BinOp::RotateLeft => 0x1E,
        BinOp::RotateRight => 0x1F,
        BinOp::WrappingAdd => 0x20,
        BinOp::WrappingSub => 0x21,
        BinOp::MulHigh => 0x22,
        BinOp::Opaque(_) => {
            return Err(EncodeError::Unsupported(
                "ExprArena V1: BinOp::Opaque extensions not yet tagged",
            ))
        }
        _ => {
            return Err(EncodeError::Unsupported(
                "Fix: BinOp variant unknown to ExprArena encoder; extend bin_op_tag",
            ))
        }
    })
}

/// Map a `UnOp` to its frozen u32 tag (per `vyre-spec/src/un_op.rs`
/// reservations).
fn un_op_tag(op: &UnOp) -> Result<u32, EncodeError> {
    Ok(match op {
        UnOp::Negate => 0x01,
        UnOp::BitNot => 0x02,
        UnOp::LogicalNot => 0x03,
        UnOp::Popcount => 0x04,
        UnOp::Clz => 0x05,
        UnOp::Ctz => 0x06,
        UnOp::ReverseBits => 0x07,
        UnOp::Cos => 0x08,
        UnOp::Sin => 0x09,
        UnOp::Abs => 0x0A,
        UnOp::Sqrt => 0x0B,
        UnOp::Floor => 0x0C,
        UnOp::Ceil => 0x0D,
        UnOp::Round => 0x0E,
        UnOp::Trunc => 0x0F,
        UnOp::Sign => 0x10,
        UnOp::IsNan => 0x11,
        UnOp::IsInf => 0x12,
        UnOp::IsFinite => 0x13,
        UnOp::Exp => 0x14,
        UnOp::Log => 0x15,
        UnOp::Log2 => 0x16,
        UnOp::Exp2 => 0x17,
        UnOp::Tan => 0x18,
        UnOp::Acos => 0x19,
        UnOp::Asin => 0x1A,
        UnOp::Atan => 0x1B,
        UnOp::Tanh => 0x1C,
        UnOp::Sinh => 0x1D,
        UnOp::Cosh => 0x1E,
        UnOp::InverseSqrt => 0x1F,
        UnOp::Unpack4Low => 0x20,
        UnOp::Unpack4High => 0x21,
        UnOp::Unpack8Low => 0x22,
        UnOp::Unpack8High => 0x23,
        UnOp::Reciprocal => 0x24,
        UnOp::Opaque(_) => {
            return Err(EncodeError::Unsupported(
                "ExprArena V1: UnOp::Opaque extensions not yet tagged",
            ))
        }
        _ => {
            return Err(EncodeError::Unsupported(
                "Fix: UnOp variant unknown to ExprArena encoder; extend un_op_tag",
            ))
        }
    })
}

/// Reverse-lookup a `bin_op_tag` u32 back into a `BinOp` for decoder
/// use. Returns `None` for unknown / extension tags.
#[must_use]

pub fn bin_op_from_tag(tag: u32) -> Option<BinOp> {
    Some(match tag {
        0x01 => BinOp::Add,
        0x02 => BinOp::Sub,
        0x03 => BinOp::Mul,
        0x04 => BinOp::Div,
        0x05 => BinOp::Mod,
        0x06 => BinOp::BitAnd,
        0x07 => BinOp::BitOr,
        0x08 => BinOp::BitXor,
        0x09 => BinOp::Shl,
        0x0A => BinOp::Shr,
        0x0B => BinOp::Eq,
        0x0C => BinOp::Ne,
        0x0D => BinOp::Lt,
        0x0E => BinOp::Gt,
        0x10 => BinOp::Le,
        0x11 => BinOp::Ge,
        0x12 => BinOp::And,
        0x13 => BinOp::Or,
        0x14 => BinOp::AbsDiff,
        0x15 => BinOp::Min,
        0x16 => BinOp::Max,
        0x17 => BinOp::SaturatingAdd,
        0x18 => BinOp::SaturatingSub,
        0x19 => BinOp::SaturatingMul,
        0x1A => BinOp::Shuffle,
        0x1B => BinOp::Ballot,
        0x1C => BinOp::WaveReduce,
        0x1D => BinOp::WaveBroadcast,
        0x1E => BinOp::RotateLeft,
        0x1F => BinOp::RotateRight,
        0x20 => BinOp::WrappingAdd,
        0x21 => BinOp::WrappingSub,
        0x22 => BinOp::MulHigh,
        _ => return None,
    })
}

/// Reverse-lookup a `un_op_tag` u32 back into a `UnOp`.
#[must_use]
pub fn un_op_from_tag(tag: u32) -> Option<UnOp> {
    Some(match tag {
        0x01 => UnOp::Negate,
        0x02 => UnOp::BitNot,
        0x03 => UnOp::LogicalNot,
        0x04 => UnOp::Popcount,
        0x05 => UnOp::Clz,
        0x06 => UnOp::Ctz,
        0x07 => UnOp::ReverseBits,
        0x08 => UnOp::Cos,
        0x09 => UnOp::Sin,
        0x0A => UnOp::Abs,
        0x0B => UnOp::Sqrt,
        0x0C => UnOp::Floor,
        0x0D => UnOp::Ceil,
        0x0E => UnOp::Round,
        0x0F => UnOp::Trunc,
        0x10 => UnOp::Sign,
        0x11 => UnOp::IsNan,
        0x12 => UnOp::IsInf,
        0x13 => UnOp::IsFinite,
        0x14 => UnOp::Exp,
        0x15 => UnOp::Log,
        0x16 => UnOp::Log2,
        0x17 => UnOp::Exp2,
        0x18 => UnOp::Tan,
        0x19 => UnOp::Acos,
        0x1A => UnOp::Asin,
        0x1B => UnOp::Atan,
        0x1C => UnOp::Tanh,
        0x1D => UnOp::Sinh,
        0x1E => UnOp::Cosh,
        0x1F => UnOp::InverseSqrt,
        0x20 => UnOp::Unpack4Low,
        0x21 => UnOp::Unpack4High,
        0x22 => UnOp::Unpack8Low,
        0x23 => UnOp::Unpack8High,
        0x24 => UnOp::Reciprocal,
        _ => return None,
    })
}

/// Helper used by passes that compute structural-hash CSE keys: walk
/// the arena and assign a stable hash to each ExprId. Children's
/// hashes are mixed in before parents'. Today this runs CPU-side; the
/// same shape ports to a level-wave GPU dispatch over `post_order`.
pub fn structural_hashes(arena: &ExprArenaEncoding) -> Vec<u64> {
    use std::hash::{Hash, Hasher};
    let mut hashes = vec![0u64; arena.expr_count as usize];
    for &id in &arena.post_order {
        let i = id as usize;
        let mut hasher = rustc_hash::FxHasher::default();
        arena.kinds[i].hash(&mut hasher);
        arena.arg0[i].hash(&mut hasher);
        // For arg1/arg2/arg3 that hold child ExprIds, mix the
        // children's already-computed hashes (post-order guarantees
        // they're populated). For literal-arg slots this just mixes
        // the value bits, which is correct since literal values are
        // payload, not pointers.
        match arena.kinds[i] {
            expr_kind::LIT_U32
            | expr_kind::LIT_I32
            | expr_kind::LIT_F32
            | expr_kind::LIT_BOOL
            | expr_kind::VAR
            | expr_kind::BUF_LEN
            | expr_kind::INVOCATION_ID
            | expr_kind::WORKGROUP_ID
            | expr_kind::LOCAL_ID
            | expr_kind::SUBGROUP_LOCAL_ID
            | expr_kind::SUBGROUP_SIZE => {}
            expr_kind::LOAD => {
                let idx = arena.arg1[i] as usize;
                hashes[idx].hash(&mut hasher);
            }
            expr_kind::BIN_OP => {
                let l = arena.arg1[i] as usize;
                let r = arena.arg2[i] as usize;
                hashes[l].hash(&mut hasher);
                hashes[r].hash(&mut hasher);
            }
            expr_kind::UN_OP => {
                let c = arena.arg1[i] as usize;
                hashes[c].hash(&mut hasher);
            }
            expr_kind::SELECT => {
                let c = arena.arg0[i] as usize;
                let t = arena.arg1[i] as usize;
                let f = arena.arg2[i] as usize;
                hashes[c].hash(&mut hasher);
                hashes[t].hash(&mut hasher);
                hashes[f].hash(&mut hasher);
            }
            expr_kind::FMA => {
                let a = arena.arg0[i] as usize;
                let b = arena.arg1[i] as usize;
                let c = arena.arg2[i] as usize;
                hashes[a].hash(&mut hasher);
                hashes[b].hash(&mut hasher);
                hashes[c].hash(&mut hasher);
            }
            _ => {}
        }
        hashes[i] = hasher.finish();
    }
    hashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimizer::encode::encode_program;
    use vyre_foundation::ir::{Expr, Node, Program};

    fn wrapped(entry: Vec<Node>) -> Program {
        Program::wrapped(Vec::new(), [1, 1, 1], entry)
    }

    #[test]
    fn empty_program_encodes_to_empty_arena() {
        let p = wrapped(Vec::new());
        let arena = encode_expr_arena(&p)
            .expect("Fix: empty optimizer program must encode into an expression arena");
        assert_eq!(arena.expr_count, 0);
        // Index 0 = ROOT (no Exprs).
        assert_eq!(arena.node_top_level_exprs.len(), 1);
    }

    #[test]
    fn lit_only_let_encodes_one_lit_expr() {
        let p = wrapped(vec![Node::let_bind("x", Expr::u32(42))]);
        let arena = encode_expr_arena(&p)
            .expect("Fix: flat let optimizer program must encode into an expression arena");
        assert_eq!(arena.expr_count, 1);
        assert_eq!(arena.kinds[0], expr_kind::LIT_U32);
        assert_eq!(arena.arg0[0], 42);
        // ROOT slot empty, then Let's value points at expr 0.
        assert_eq!(arena.node_top_level_exprs[0].len(), 0);
        assert_eq!(arena.node_top_level_exprs[1], vec![0]);
    }

    #[test]
    fn binop_emits_post_order_left_right_parent() {
        // let x = u32(2) + u32(3)
        let p = wrapped(vec![Node::let_bind(
            "x",
            Expr::add(Expr::u32(2), Expr::u32(3)),
        )]);
        let arena = encode_expr_arena(&p)
            .expect("Fix: binop optimizer program must encode into an expression arena");
        assert_eq!(arena.expr_count, 3);
        // Children come first in post-order.
        assert_eq!(arena.kinds[0], expr_kind::LIT_U32);
        assert_eq!(arena.arg0[0], 2);
        assert_eq!(arena.kinds[1], expr_kind::LIT_U32);
        assert_eq!(arena.arg0[1], 3);
        assert_eq!(arena.kinds[2], expr_kind::BIN_OP);
        assert_eq!(arena.arg0[2], 0x01); // Add
        assert_eq!(arena.arg1[2], 0); // left = u32(2)
        assert_eq!(arena.arg2[2], 1); // right = u32(3)
                                      // Top-level Expr for the Let is the BinOp (id 2).
        assert_eq!(arena.node_top_level_exprs[1], vec![2]);
    }

    #[test]
    fn nested_if_records_each_node_top_level_exprs() {
        // if c { let inner = u32(7) } else {}
        let p = wrapped(vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("inner", Expr::u32(7))],
            otherwise: vec![],
        }]);
        let arena = encode_expr_arena(&p)
            .expect("Fix: nested if optimizer program must encode into an expression arena");
        // Exprs in encounter order:
        //   0: Var("c")         -  If's cond
        //   1: LitU32(7)        -  let inner's value
        assert_eq!(arena.expr_count, 2);
        // node_top_level_exprs:
        //   [0]: ROOT (empty)
        //   [1]: If (cond = 0)
        //   [2]: let inner (value = 1)
        assert_eq!(arena.node_top_level_exprs[0], Vec::<u32>::new());
        assert_eq!(arena.node_top_level_exprs[1], vec![0]);
        assert_eq!(arena.node_top_level_exprs[2], vec![1]);
    }

    #[test]
    fn arena_node_slots_match_program_graph_node_count() {
        let p = wrapped(vec![
            Node::let_bind("root_value", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::If {
                cond: Expr::var("predicate"),
                then: vec![
                    Node::let_bind("then_value", Expr::mul(Expr::u32(3), Expr::u32(4))),
                    Node::Block(vec![Node::let_bind(
                        "blocked_value",
                        Expr::sub(Expr::u32(9), Expr::u32(5)),
                    )]),
                ],
                otherwise: vec![Node::let_bind(
                    "else_value",
                    Expr::add(Expr::var("root_value"), Expr::u32(1)),
                )],
            },
        ]);

        let arena = encode_expr_arena(&p).expect("Fix: expression arena encoding must succeed");
        let graph = encode_program(&p).expect("Fix: program graph encoding must succeed");

        assert_eq!(
            arena.node_top_level_exprs.len() as u32,
            graph.node_count,
            "resident GPU validation depends on arena node slots matching ProgramGraph node ids"
        );
    }

    #[test]
    fn structural_hashes_collide_for_equal_subexprs() {
        // let x = (a + b)
        // let y = (a + b)
        // The two BinOp Exprs are structurally identical → same hash.
        let p = wrapped(vec![
            Node::let_bind("x", Expr::add(Expr::var("a"), Expr::var("b"))),
            Node::let_bind("y", Expr::add(Expr::var("a"), Expr::var("b"))),
        ]);
        let arena = encode_expr_arena(&p)
            .expect("Fix: dual binop optimizer program must encode into an expression arena");
        // 6 Exprs: var(a), var(b), add  -  twice.
        assert_eq!(arena.expr_count, 6);
        let hashes = structural_hashes(&arena);
        // The two add Exprs are at ids 2 and 5.
        assert_eq!(
            hashes[2], hashes[5],
            "structurally-equal BinOps must share a hash"
        );
        // The two Var(a) Exprs are at ids 0 and 3.
        assert_eq!(hashes[0], hashes[3], "Var(a) hashes match");
        // Var(a) and Var(b) must NOT collide.
        assert_ne!(hashes[0], hashes[1]);
    }

    #[test]
    fn structural_hashes_distinguish_commutative_operand_order() {
        // a + b vs b + a  -  without a canonicalize pass, these still
        // produce different structural hashes (left/right matter).
        let p = wrapped(vec![
            Node::let_bind("x", Expr::add(Expr::var("a"), Expr::var("b"))),
            Node::let_bind("y", Expr::add(Expr::var("b"), Expr::var("a"))),
        ]);
        let arena = encode_expr_arena(&p)
            .expect("Fix: dual binop optimizer program must encode into an expression arena");
        let hashes = structural_hashes(&arena);
        assert_eq!(arena.expr_count, 6);
        // First add is at id 2, second at id 5.
        assert_ne!(
            hashes[2], hashes[5],
            "operand order is part of the structural hash until canonicalize runs"
        );
    }

    #[test]
    fn unsupported_expr_returns_unsupported_error() {
        // Expr::Cast is not yet encoded.
        let p = wrapped(vec![Node::let_bind(
            "x",
            Expr::Cast {
                target: vyre_foundation::ir::DataType::U32,
                value: Box::new(Expr::u32(7)),
            },
        )]);
        let err = encode_expr_arena(&p).expect_err("cast not supported in V1");
        assert!(matches!(err, EncodeError::Unsupported(_)));
    }
}
