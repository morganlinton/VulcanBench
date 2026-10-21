use super::{
    ProgramStats, CAP_ASYNC_DISPATCH, CAP_BF16, CAP_DISTRIBUTED_COLLECTIVES, CAP_F16, CAP_F64,
    CAP_INDIRECT_DISPATCH, CAP_SUBGROUP_OPS, CAP_TENSOR_OPS, CAP_TRAP, NODE_KIND_ASSIGN,
    NODE_KIND_BARRIER, NODE_KIND_IF, NODE_KIND_LET, NODE_KIND_LOOP, NODE_KIND_REGION,
    NODE_KIND_STORE,
};

impl ProgramStats {
    /// True when the program uses subgroup operations.
    #[inline]
    #[must_use]
    pub fn subgroup_ops(&self) -> bool {
        self.capability_bits & CAP_SUBGROUP_OPS != 0
    }

    /// True when the program uses IEEE-754 binary16 values.
    #[inline]
    #[must_use]
    pub fn f16(&self) -> bool {
        self.capability_bits & CAP_F16 != 0
    }

    /// True when the program uses bfloat16 values.
    #[inline]
    #[must_use]
    pub fn bf16(&self) -> bool {
        self.capability_bits & CAP_BF16 != 0
    }

    /// True when the program uses IEEE-754 binary64 values.
    #[inline]
    #[must_use]
    pub fn f64(&self) -> bool {
        self.capability_bits & CAP_F64 != 0
    }

    /// True when the program requires async dispatch semantics.
    #[inline]
    #[must_use]
    pub fn async_dispatch(&self) -> bool {
        self.capability_bits & CAP_ASYNC_DISPATCH != 0
    }

    /// True when the program requires indirect dispatch support.
    #[inline]
    #[must_use]
    pub fn indirect_dispatch(&self) -> bool {
        self.capability_bits & CAP_INDIRECT_DISPATCH != 0
    }

    /// True when the program uses tensor / tensor-core operand types.
    #[inline]
    #[must_use]
    pub fn tensor_ops(&self) -> bool {
        self.capability_bits & CAP_TENSOR_OPS != 0
    }

    /// True when the program uses `Node::Trap`.
    #[inline]
    #[must_use]
    pub fn trap(&self) -> bool {
        self.capability_bits & CAP_TRAP != 0
    }

    /// True when the program uses distributed collective communication nodes.
    #[inline]
    #[must_use]
    pub fn distributed_collectives(&self) -> bool {
        self.capability_bits & CAP_DISTRIBUTED_COLLECTIVES != 0
    }

    /// True when at least one node of any kind in `mask` was observed
    /// in the stats walk. Use the `NODE_KIND_*` constants to compose
    /// the mask:
    ///
    /// ```ignore
    /// use vyre_foundation::ir::stats::{NODE_KIND_LOOP, NODE_KIND_IF};
    /// if program.stats().has_any_node_kind(NODE_KIND_LOOP | NODE_KIND_IF) {
    ///     // walk the tree
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn has_any_node_kind(&self, mask: u32) -> bool {
        (self.node_kinds_present & mask) != 0
    }

    /// True when the program contains at least one `Node::Let`.
    #[inline]
    #[must_use]
    pub fn has_node_let(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_LET)
    }
    /// True when the program contains at least one `Node::Loop`.
    #[inline]
    #[must_use]
    pub fn has_node_loop(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_LOOP)
    }
    /// True when the program contains at least one `Node::If`.
    #[inline]
    #[must_use]
    pub fn has_node_if(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_IF)
    }
    /// True when the program contains at least one `Node::Store`.
    #[inline]
    #[must_use]
    pub fn has_node_store(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_STORE)
    }
    /// True when the program contains at least one `Node::Barrier`.
    #[inline]
    #[must_use]
    pub fn has_node_barrier(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_BARRIER)
    }
    /// True when the program contains at least one `Node::Assign`.
    #[inline]
    #[must_use]
    pub fn has_node_assign(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_ASSIGN)
    }
    /// True when the program contains at least one `Node::Region`.
    #[inline]
    #[must_use]
    pub fn has_node_region(&self) -> bool {
        self.has_any_node_kind(NODE_KIND_REGION)
    }
}
