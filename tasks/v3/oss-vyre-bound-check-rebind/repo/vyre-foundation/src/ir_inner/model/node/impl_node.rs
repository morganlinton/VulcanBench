use super::{Node, NodeExtension};
use crate::ir_inner::model::expr::{Expr, Ident};
use crate::memory_model::MemoryOrdering;
use std::sync::Arc;

impl Node {
    /// `let name = value;`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::let_bind("x", Expr::u32(1));
    /// ```
    #[must_use]
    #[inline]
    pub fn let_bind(name: impl Into<Ident>, value: Expr) -> Self {
        Self::Let {
            name: name.into(),
            value,
        }
    }

    /// `name = value;`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::assign("x", Expr::u32(2));
    /// ```
    #[must_use]
    #[inline]
    pub fn assign(name: impl Into<Ident>, value: Expr) -> Self {
        Self::Assign {
            name: name.into(),
            value,
        }
    }

    /// `buffer[index] = value;`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::store("out", Expr::u32(0), Expr::u32(1));
    /// ```
    #[must_use]
    #[inline]
    pub fn store(buffer: impl Into<Ident>, index: Expr, value: Expr) -> Self {
        Self::Store {
            buffer: buffer.into(),
            index,
            value,
        }
    }

    /// `if cond { then } else { otherwise }`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::if_then_else(Expr::bool(true), vec![Node::Return], vec![]);
    /// ```
    #[must_use]
    #[inline]
    pub fn if_then_else(cond: Expr, then: Vec<Self>, otherwise: Vec<Self>) -> Self {
        Self::If {
            cond,
            then,
            otherwise,
        }
    }

    /// `if cond { then }`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::if_then(Expr::bool(true), vec![Node::Return]);
    /// ```
    #[must_use]
    #[inline]
    pub fn if_then(cond: Expr, then: Vec<Self>) -> Self {
        Self::If {
            cond,
            then,
            otherwise: Vec::new(),
        }
    }

    /// `for var in from..to { body }`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    /// let _ = Node::loop_for("i", Expr::u32(0), Expr::u32(4), vec![]);
    /// ```
    #[must_use]
    #[inline]
    pub fn loop_for(var: impl Into<Ident>, from: Expr, to: Expr, body: Vec<Self>) -> Self {
        Self::Loop {
            var: var.into(),
            from,
            to,
            body,
        }
    }

    /// `for var in from..to { body }`
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Expr, Node};
    ///
    /// let node = Node::loop_("i", Expr::u32(0), Expr::u32(4), vec![Node::Return]);
    /// assert!(matches!(node, Node::Loop { .. }));
    /// ```
    #[must_use]
    #[inline]
    pub fn loop_(var: impl Into<Ident>, from: Expr, to: Expr, body: Vec<Self>) -> Self {
        Self::loop_for(var, from, to, body)
    }

    /// Effectively-infinite loop used by persistent kernels (megakernel,
    /// event loops, streaming). Lowers to `Node::Loop` with
    /// `from: 0, to: u32::MAX`. At 1 µs per iteration `u32::MAX` is ~68
    /// years  -  for all practical purposes infinite. The inner body
    /// drives termination via `Node::Return` or by observing an
    /// atomic shutdown flag the host sets.
    ///
    /// Linus principle: one enum variant (`Node::Loop`) handles both
    /// bounded and persistent cases. No cascade of match arms through
    /// every pass; no new wire-format tag. An optimizer pass
    /// that wants to distinguish "truly unbounded" from "large bound"
    /// inspects the `to` expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// let persistent = Node::forever(vec![Node::Return]);
    /// assert!(matches!(persistent, Node::Loop { .. }));
    /// ```
    #[must_use]
    #[inline]
    pub fn forever(body: Vec<Self>) -> Self {
        Self::loop_for("__forever__", Expr::u32(0), Expr::u32(u32::MAX), body)
    }

    /// Sequence of statements.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// assert!(matches!(Node::block(vec![Node::Return]), Node::Block(_)));
    /// ```
    #[must_use]
    #[inline]
    pub fn block(nodes: Vec<Self>) -> Self {
        Self::Block(nodes)
    }

    /// Early return from the entry point.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// assert!(matches!(Node::return_(), Node::Return));
    /// ```
    #[must_use]
    pub const fn return_() -> Self {
        Self::Return
    }

    /// Workgroup barrier statement.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// assert!(matches!(Node::barrier(), Node::Barrier { .. }));
    /// ```
    #[must_use]
    pub const fn barrier() -> Self {
        Self::Barrier {
            ordering: MemoryOrdering::SeqCst,
        }
    }

    /// Workgroup barrier statement with explicit memory ordering.
    #[must_use]
    pub const fn barrier_with_ordering(ordering: MemoryOrdering) -> Self {
        Self::Barrier { ordering }
    }

    /// Statement-level invocation of another registered op by stable
    /// op id.
    ///
    /// Represented as a named [`Node::Region`] whose `generator` is
    /// the callee's op id and whose body is an internal sequence of
    /// `Node::Let { name: "arg{i}", value: <arg_expr> }` bindings.
    /// Every backend already handles `Node::Region`  -  the op-registry
    /// inliner walks the arg binds, substitutes them into the callee's
    /// fragment, and splices the result in place. No new IR variant
    /// is introduced, and the arg values remain fully visible to CSE,
    /// DCE, and constant folding through the let-chain.
    #[must_use]
    pub fn call(op_id: impl Into<Ident>, args: Vec<Expr>) -> Self {
        let body: Vec<Node> = args
            .into_iter()
            .enumerate()
            .map(|(idx, expr)| Node::let_bind(format!("arg{idx}"), expr))
            .collect();
        Self::Region {
            generator: op_id.into(),
            source_region: None,
            body: Arc::new(body),
        }
    }

    /// Command-level indirect dispatch metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// let node = Node::indirect_dispatch("counts", 0);
    /// assert!(matches!(node, Node::IndirectDispatch { .. }));
    /// ```
    #[must_use]
    #[inline]
    pub fn indirect_dispatch(count_buffer: impl Into<Ident>, count_offset: u64) -> Self {
        Self::IndirectDispatch {
            count_buffer: count_buffer.into(),
            count_offset,
        }
    }

    /// Begin an asynchronous transfer stream region (GPU-driven).
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{Node, Expr};
    ///
    /// let node = Node::async_load_ext("ssd", "vram", Expr::u32(0), Expr::u32(1024), "tag-0");
    /// assert!(matches!(node, Node::AsyncLoad { .. }));
    /// ```
    #[must_use]
    #[inline]
    pub fn async_load_ext(
        source: impl Into<Ident>,
        destination: impl Into<Ident>,
        offset: Expr,
        size: Expr,
        tag: impl Into<Ident>,
    ) -> Self {
        Self::AsyncLoad {
            source: source.into(),
            destination: destination.into(),
            offset: Box::new(offset),
            size: Box::new(size),
            tag: tag.into(),
        }
    }

    /// Begin an asynchronous transfer stream region (legacy/host-driven).
    #[must_use]
    #[inline]
    pub fn async_load(tag: impl Into<Ident>) -> Self {
        Self::async_load_ext(
            "__legacy_src__",
            "__legacy_dst__",
            Expr::u32(0),
            Expr::u32(0),
            tag,
        )
    }

    /// Begin an asynchronous store transfer stream region (GPU-driven).
    #[must_use]
    #[inline]
    pub fn async_store(
        source: impl Into<Ident>,
        destination: impl Into<Ident>,
        offset: Expr,
        size: Expr,
        tag: impl Into<Ident>,
    ) -> Self {
        Self::AsyncStore {
            source: source.into(),
            destination: destination.into(),
            offset: Box::new(offset),
            size: Box::new(size),
            tag: tag.into(),
        }
    }

    /// Wait for an asynchronous transfer stream region.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Node;
    ///
    /// let node = Node::async_wait("stage-a");
    /// assert!(matches!(node, Node::AsyncWait { .. }));
    /// ```
    #[must_use]
    #[inline]
    pub fn async_wait(tag: impl Into<Ident>) -> Self {
        Self::AsyncWait { tag: tag.into() }
    }

    /// Trap the current execution lane (GPU-initiated page fault).
    #[must_use]
    #[inline]
    pub fn trap(address: Expr, tag: impl Into<Ident>) -> Self {
        Self::Trap {
            address: Box::new(address),
            tag: tag.into(),
        }
    }

    /// Resume a previously trapped execution lane.
    #[must_use]
    #[inline]
    pub fn resume(tag: impl Into<Ident>) -> Self {
        Self::Resume { tag: tag.into() }
    }

    /// Wrap a downstream extension statement node.
    #[must_use]
    #[inline]
    pub fn opaque(node: impl NodeExtension) -> Self {
        Self::Opaque(Arc::new(node))
    }

    /// Wrap a shared downstream extension statement node.
    #[must_use]
    #[inline]
    pub fn opaque_arc(node: Arc<dyn NodeExtension>) -> Self {
        Self::Opaque(node)
    }
}
