#![allow(missing_docs)]

use crate::ir_inner::model::expr::{ExprNode, GeneratorRef, Ident};
use crate::ir_inner::model::node::NodeExtension;
use crate::ir_inner::model::types::{
    AtomicOp, BinOp, CollectiveOp, CommGroup, DataType, SubgroupReduceOp, UnOp,
};
use std::sync::Arc;

vyre_macros::vyre_ast_registry! {
    /// Statement nodes  -  execute effects.
    Node {
        Let { name: Ident, value: Expr },
        Assign { name: Ident, value: Expr },
        Store { buffer: Ident, index: Expr, value: Expr },
        If { cond: Expr, then: Vec<Node>, otherwise: Vec<Node> },
        Loop { var: Ident, from: Expr, to: Expr, body: Vec<Node> },
        IndirectDispatch { count_buffer: Ident, count_offset: u64 },
        AsyncLoad { source: Ident, destination: Ident, offset: Box<Expr>, size: Box<Expr>, tag: Ident },
        AsyncStore { source: Ident, destination: Ident, offset: Box<Expr>, size: Box<Expr>, tag: Ident },
        AsyncWait { tag: Ident },
        Trap { address: Box<Expr>, tag: Ident },
        Resume { tag: Ident },
        AllReduce { buffer: Ident, op: CollectiveOp, group: CommGroup },
        AllGather { input: Ident, output: Ident, group: CommGroup },
        ReduceScatter { input: Ident, output: Ident, op: CollectiveOp, group: CommGroup },
        Broadcast { buffer: Ident, root: u32, group: CommGroup },
        Return,
        Barrier { ordering: crate::memory_model::MemoryOrdering },
        Block(Vec<Node>),
        Region { generator: Ident, source_region: Option<GeneratorRef>, body: Arc<Vec<Node>> },
        Opaque(Arc<dyn NodeExtension>),
    }

    /// Expression nodes  -  produce values.
    Expr {
        LitU32(u32),
        LitI32(i32),
        LitF32(f32),
        LitBool(bool),
        Var(Ident),
        Load { buffer: Ident, index: Box<Expr> },
        BufLen { buffer: Ident },
        InvocationId { axis: u8 },
        WorkgroupId { axis: u8 },
        LocalId { axis: u8 },
        BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
        UnOp { op: UnOp, operand: Box<Expr> },
        Call { op_id: Ident, args: Vec<Expr> },
        Select { cond: Box<Expr>, true_val: Box<Expr>, false_val: Box<Expr> },
        Cast { target: DataType, value: Box<Expr> },
        Fma { a: Box<Expr>, b: Box<Expr>, c: Box<Expr> },
        Atomic { op: AtomicOp, buffer: Ident, index: Box<Expr>, expected: Option<Box<Expr>>, value: Box<Expr>, ordering: crate::memory_model::MemoryOrdering },
        SubgroupBallot { cond: Box<Expr> },
        SubgroupShuffle { value: Box<Expr>, lane: Box<Expr> },
        SubgroupReduce { op: SubgroupReduceOp, value: Box<Expr> },
        SubgroupLocalId,
        SubgroupSize,
        Opaque(Arc<dyn ExprNode>),
    }
}
