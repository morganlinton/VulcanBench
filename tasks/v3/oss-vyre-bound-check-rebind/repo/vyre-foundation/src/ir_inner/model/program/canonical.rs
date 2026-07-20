use std::sync::Arc;

use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::types::BinOp;

use super::{meta::buffer_decl_canonical_key, BufferDecl, Program};

impl Program {
    /// Return the canonical IR shape used for security-sensitive cache keys.
    ///
    /// Canonicalization preserves executable semantics while removing
    /// authoring-order noise: buffer declarations are sorted by their stable
    /// wire key, commutative expression operands are normalized, and `Block`
    /// wrappers that do not own local bindings are flattened.
    #[must_use]
    pub fn canonicalized(&self) -> Self {
        let mut buffers = self.buffers().to_vec();
        sort_buffers(&mut buffers);
        let mut ctx = CanonicalCtx::default();
        self.with_rewritten_entry(ctx.canonicalize_nodes(self.entry()))
            .with_rewritten_buffers(buffers)
    }

    /// Serialize the canonical IR shape into stable VIR0 wire bytes.
    ///
    /// # Errors
    ///
    /// Returns the same wire-format validation errors as [`Self::to_wire`],
    /// but after canonical normalization has been applied.
    #[must_use]
    pub fn canonical_wire_bytes(&self) -> Result<Vec<u8>, crate::error::Error> {
        let canonical = self.canonicalized();
        // Pre-size: VIR0 wire encoding lands in the ballpark of ~32
        // bytes per IR node + a fixed program header. Over-sizing is
        // free at this stage and avoids the typical 4-7 reallocations
        // a fresh Vec<u8> would do while the encoder pushes header
        // tags + buffer table + node tree.
        let stats = canonical.stats();
        let estimate = 256
            + stats.node_count.saturating_mul(48)
            + canonical.buffers().len().saturating_mul(64);
        let mut out = Vec::with_capacity(estimate);
        crate::serial::wire::encode::to_wire_into(&canonical, &mut out)
            .map_err(|message| crate::error::Error::WireFormatValidation { message })?;
        Ok(out)
    }

    /// BLAKE3 digest of [`Self::canonical_wire_bytes`].
    ///
    /// # Errors
    ///
    /// Returns a wire-format validation error if the canonical program cannot
    /// be represented by the current VIR0 encoder.
    pub fn canonical_wire_hash(&self) -> Result<blake3::Hash, crate::error::Error> {
        self.canonical_wire_bytes()
            .map(|bytes| blake3::hash(&bytes))
    }
}

fn sort_buffers(buffers: &mut [BufferDecl]) {
    buffers.sort_by_cached_key(buffer_decl_canonical_key);
}

#[derive(Default)]
struct CanonicalCtx {
    left_key: Vec<u8>,
    right_key: Vec<u8>,
}

impl CanonicalCtx {
    fn canonicalize_nodes(&mut self, nodes: &[Node]) -> Vec<Node> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            push_canonical_node(&mut out, self.canonicalize_node(node));
        }
        out
    }

    fn canonicalize_node(&mut self, node: &Node) -> Node {
        match node {
            Node::Let { name, value } => Node::Let {
                name: name.clone(),
                value: self.canonicalize_expr(value),
            },
            Node::Assign { name, value } => Node::Assign {
                name: name.clone(),
                value: self.canonicalize_expr(value),
            },
            Node::Store {
                buffer,
                index,
                value,
            } => Node::Store {
                buffer: buffer.clone(),
                index: self.canonicalize_expr(index),
                value: self.canonicalize_expr(value),
            },
            Node::If {
                cond,
                then,
                otherwise,
            } => Node::If {
                cond: self.canonicalize_expr(cond),
                then: self.canonicalize_nodes(then),
                otherwise: self.canonicalize_nodes(otherwise),
            },
            Node::Loop {
                var,
                from,
                to,
                body,
            } => Node::Loop {
                var: var.clone(),
                from: self.canonicalize_expr(from),
                to: self.canonicalize_expr(to),
                body: self.canonicalize_nodes(body),
            },
            Node::Block(children) => Node::Block(self.canonicalize_nodes(children)),
            Node::Region {
                generator,
                source_region,
                body,
            } => Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(self.canonicalize_nodes(body)),
            },
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => Node::AsyncLoad {
                source: source.clone(),
                destination: destination.clone(),
                offset: Box::new(self.canonicalize_expr(offset)),
                size: Box::new(self.canonicalize_expr(size)),
                tag: tag.clone(),
            },
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => Node::AsyncStore {
                source: source.clone(),
                destination: destination.clone(),
                offset: Box::new(self.canonicalize_expr(offset)),
                size: Box::new(self.canonicalize_expr(size)),
                tag: tag.clone(),
            },
            Node::Trap { address, tag } => Node::Trap {
                address: Box::new(self.canonicalize_expr(address)),
                tag: tag.clone(),
            },
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => Node::IndirectDispatch {
                count_buffer: count_buffer.clone(),
                count_offset: *count_offset,
            },
            Node::AllReduce { buffer, op, group } => Node::AllReduce {
                buffer: buffer.clone(),
                op: *op,
                group: *group,
            },
            Node::AllGather {
                input,
                output,
                group,
            } => Node::AllGather {
                input: input.clone(),
                output: output.clone(),
                group: *group,
            },
            Node::ReduceScatter {
                input,
                output,
                op,
                group,
            } => Node::ReduceScatter {
                input: input.clone(),
                output: output.clone(),
                op: *op,
                group: *group,
            },
            Node::Broadcast {
                buffer,
                root,
                group,
            } => Node::Broadcast {
                buffer: buffer.clone(),
                root: *root,
                group: *group,
            },
            Node::AsyncWait { tag } => Node::AsyncWait { tag: tag.clone() },
            Node::Resume { tag } => Node::Resume { tag: tag.clone() },
            Node::Return => Node::Return,
            Node::Barrier { ordering } => Node::barrier_with_ordering(*ordering),
            Node::Opaque(extension) => Node::Opaque(Arc::clone(extension)),
        }
    }

    fn canonicalize_expr(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::BinOp { op, left, right } => {
                let mut left = self.canonicalize_expr(left);
                let mut right = self.canonicalize_expr(right);
                if should_swap_operands(*op, &left, &right, &mut self.left_key, &mut self.right_key)
                {
                    std::mem::swap(&mut left, &mut right);
                }
                Expr::BinOp {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }
            Expr::UnOp { op, operand } => Expr::UnOp {
                op: op.clone(),
                operand: Box::new(self.canonicalize_expr(operand)),
            },
            Expr::Load { buffer, index } => Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(self.canonicalize_expr(index)),
            },
            Expr::Call { op_id, args } => Expr::Call {
                op_id: op_id.clone(),
                args: args.iter().map(|arg| self.canonicalize_expr(arg)).collect(),
            },
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => Expr::Select {
                cond: Box::new(self.canonicalize_expr(cond)),
                true_val: Box::new(self.canonicalize_expr(true_val)),
                false_val: Box::new(self.canonicalize_expr(false_val)),
            },
            Expr::Cast { target, value } => Expr::Cast {
                target: target.clone(),
                value: Box::new(self.canonicalize_expr(value)),
            },
            Expr::Fma { a, b, c } => Expr::Fma {
                a: Box::new(self.canonicalize_expr(a)),
                b: Box::new(self.canonicalize_expr(b)),
                c: Box::new(self.canonicalize_expr(c)),
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
                buffer: buffer.clone(),
                index: Box::new(self.canonicalize_expr(index)),
                expected: expected
                    .as_ref()
                    .map(|expr| Box::new(self.canonicalize_expr(expr))),
                value: Box::new(self.canonicalize_expr(value)),
                ordering: *ordering,
            },
            Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
                cond: Box::new(self.canonicalize_expr(cond)),
            },
            Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
                value: Box::new(self.canonicalize_expr(value)),
                lane: Box::new(self.canonicalize_expr(lane)),
            },
            Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce {
                op: *op,
                value: Box::new(self.canonicalize_expr(value)),
            },
            other => other.clone(),
        }
    }
}

fn push_canonical_node(out: &mut Vec<Node>, node: Node) {
    match node {
        Node::Block(children) if can_splice_block(&children) => out.extend(children),
        other => out.push(other),
    }
}

fn can_splice_block(nodes: &[Node]) -> bool {
    nodes.iter().all(|node| !matches!(node, Node::Let { .. }))
}

fn should_swap_operands(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    left_key: &mut Vec<u8>,
    right_key: &mut Vec<u8>,
) -> bool {
    if !is_commutative_binop(op) {
        return false;
    }
    match (is_literal(left), is_literal(right)) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => {
            // Both literals: every commutative op is observably-safe
            // to canonicalize because the literal pair folds to the
            // same value regardless of order. The float-sensitivity
            // contract (Add/Mul reassociation changes rounding) only
            // applies when at least one operand is non-literal.
            expr_wire_key_cmp(left, right, left_key, right_key).is_gt()
        }
        (false, false) => {
            can_sort_all_operands(op) && expr_wire_key_cmp(left, right, left_key, right_key).is_gt()
        }
    }
}

fn expr_wire_key_cmp(
    left: &Expr,
    right: &Expr,
    left_key: &mut Vec<u8>,
    right_key: &mut Vec<u8>,
) -> std::cmp::Ordering {
    left_key.clear();
    right_key.clear();
    append_expr_wire_key(left_key, left);
    append_expr_wire_key(right_key, right);
    left_key.as_slice().cmp(right_key.as_slice())
}

fn append_expr_wire_key(key: &mut Vec<u8>, expr: &Expr) {
    if let Err(error) = crate::serial::wire::encode::put_expr(key, expr) {
        key.clear();
        key.extend_from_slice(b"VYRE-CANONICAL-EXPR-WIRE-ERROR\0");
        key.extend_from_slice(error.as_bytes());
    }
}

fn is_commutative_binop(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::WrappingAdd
            | BinOp::SaturatingAdd
            | BinOp::Mul
            | BinOp::SaturatingMul
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
            | BinOp::Min
            | BinOp::Max
            | BinOp::AbsDiff
    )
}

fn can_sort_all_operands(op: BinOp) -> bool {
    // Ops whose operand swap is observably safe even when both
    // operands are arbitrary non-literal expressions. Excludes Add /
    // Mul because float reassociation changes rounding for non-literal
    // chains; `should_swap_operands` handles the both-literal case
    // separately so the canonical fingerprint still normalises
    // `Add(1, 2)` vs `Add(2, 1)`.
    matches!(
        op,
        BinOp::WrappingAdd
            | BinOp::SaturatingAdd
            | BinOp::SaturatingMul
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
            | BinOp::AbsDiff
    )
}

fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitF32(_) | Expr::LitBool(_)
    )
}
