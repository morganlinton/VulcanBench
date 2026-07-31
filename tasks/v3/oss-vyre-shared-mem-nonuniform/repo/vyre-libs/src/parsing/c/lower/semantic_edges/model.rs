use crate::parsing::c::lower::C_AST_PG_EDGE_NONE;

pub(crate) const VAST_NODE_STRIDE_U32: u32 = 10;
pub(crate) const IDX_KIND: usize = 0;
pub(crate) const IDX_PARENT: usize = 1;
pub(crate) const IDX_FIRST_CHILD: usize = 2;
pub(crate) const IDX_NEXT_SIBLING: usize = 3;
pub(crate) const IDX_SYMBOL_HASH: usize = 9;
pub(crate) const COMMON_PARENT_WALK_LIMIT: u32 = 64;

#[derive(Clone, Copy)]
pub(crate) struct SemanticEdge {
    pub(crate) kind: u32,
    pub(crate) src: u32,
    pub(crate) dst: u32,
}

impl SemanticEdge {
    pub(crate) const NONE: Self = Self {
        kind: C_AST_PG_EDGE_NONE,
        src: u32::MAX,
        dst: u32::MAX,
    };

    pub(crate) const fn new(kind: u32, src: u32, dst: u32) -> Self {
        Self { kind, src, dst }
    }
}
