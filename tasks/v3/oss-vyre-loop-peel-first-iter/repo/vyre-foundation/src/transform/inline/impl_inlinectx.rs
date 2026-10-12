use super::expand::CalleeExpander;
use super::{
    input_arg_map, input_buffers, output_buffer, zero_value, Error, Expr, HashMap, Ident,
    InlineCtx, Node, OpResolver, Program, Result,
};

impl InlineCtx {
    #[inline]
    pub(crate) fn new(resolver: OpResolver) -> Self {
        Self {
            resolver,
            stack: Vec::new(),
            next_call_id: 0,
        }
    }

    #[inline]
    pub(crate) fn inline_nodes(&mut self, nodes: &[Node]) -> Result<Vec<Node>> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            out.extend(self.inline_node(node)?);
        }
        Ok(out)
    }

    #[inline]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive Node inlining dispatch keeps each IR variant's rewrite contract visible"
    )]
    pub(crate) fn inline_node(&mut self, node: &Node) -> Result<Vec<Node>> {
        match node {
            Node::Let { name, value } => {
                let (mut prefix, value) = self.inline_expr(value)?;
                prefix.push(Node::let_bind(name, value));
                Ok(prefix)
            }
            Node::Assign { name, value } => {
                let (mut prefix, value) = self.inline_expr(value)?;
                prefix.push(Node::assign(name, value));
                Ok(prefix)
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                let (mut prefix, index) = self.inline_expr(index)?;
                let (value_prefix, value) = self.inline_expr(value)?;
                prefix.extend(value_prefix);
                prefix.push(Node::store(buffer, index, value));
                Ok(prefix)
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let (mut prefix, cond) = self.inline_expr(cond)?;
                prefix.push(Node::if_then_else(
                    cond,
                    self.inline_nodes(then)?,
                    self.inline_nodes(otherwise)?,
                ));
                Ok(prefix)
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let (mut prefix, from) = self.inline_expr(from)?;
                let (to_prefix, to) = self.inline_expr(to)?;
                prefix.extend(to_prefix);
                prefix.push(Node::loop_for(var, from, to, self.inline_nodes(body)?));
                Ok(prefix)
            }
            Node::Return => Ok(vec![Node::Return]),
            Node::Block(nodes) => Ok(vec![Node::Block(self.inline_nodes(nodes)?)]),
            Node::Barrier { ordering } => Ok(vec![Node::barrier_with_ordering(*ordering)]),
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => Ok(vec![Node::IndirectDispatch {
                count_buffer: count_buffer.clone(),
                count_offset: *count_offset,
            }]),
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => Ok(vec![Node::async_load_ext(
                source.clone(),
                destination.clone(),
                (**offset).clone(),
                (**size).clone(),
                tag.clone(),
            )]),
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => Ok(vec![Node::async_store(
                source.clone(),
                destination.clone(),
                (**offset).clone(),
                (**size).clone(),
                tag.clone(),
            )]),
            Node::AsyncWait { tag } => Ok(vec![Node::async_wait(tag)]),
            Node::Trap { .. }
            | Node::Resume { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. } => Ok(vec![node.clone()]),
            Node::Region {
                generator,
                source_region,
                body,
            } => Ok(vec![Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: std::sync::Arc::new(self.inline_nodes(body)?),
            }]),
            Node::Opaque(extension) => Err(Error::Interp {
                message: format!(
                    "inliner cannot rewrite opaque statement extension `{}`/`{}`. Fix: lower the extension to core Node variants before inlining.",
                    extension.extension_kind(),
                    extension.debug_identity()
                ),
            }),
        }
    }

    #[inline]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive Expr inlining dispatch keeps prefix-emission ordering auditable"
    )]
    pub(crate) fn inline_expr(&mut self, expr: &Expr) -> Result<(Vec<Node>, Expr)> {
        match expr {
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. }
            | Expr::Opaque(_) => Ok((Vec::new(), expr.clone())),
            Expr::Load { buffer, index } => {
                let (prefix, index) = self.inline_expr(index)?;
                Ok((
                    prefix,
                    Expr::Load {
                        buffer: buffer.clone(),
                        index: Box::new(index),
                    },
                ))
            }
            Expr::BinOp { op, left, right } => {
                let (mut prefix, left) = self.inline_expr(left)?;
                let (right_prefix, right) = self.inline_expr(right)?;
                prefix.extend(right_prefix);
                Ok((
                    prefix,
                    Expr::BinOp {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                ))
            }
            Expr::UnOp { op, operand } => {
                let (prefix, operand) = self.inline_expr(operand)?;
                Ok((
                    prefix,
                    Expr::UnOp {
                        op: op.clone(),
                        operand: Box::new(operand),
                    },
                ))
            }
            Expr::Fma { a, b, c } => {
                let (mut prefix, a) = self.inline_expr(a)?;
                let (b_prefix, b) = self.inline_expr(b)?;
                let (c_prefix, c) = self.inline_expr(c)?;
                prefix.extend(b_prefix);
                prefix.extend(c_prefix);
                Ok((
                    prefix,
                    Expr::Fma {
                        a: Box::new(a),
                        b: Box::new(b),
                        c: Box::new(c),
                    },
                ))
            }
            Expr::Call { op_id, args } => self.inline_call(op_id, args),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                let (mut prefix, cond) = self.inline_expr(cond)?;
                let (true_prefix, true_val) = self.inline_expr(true_val)?;
                let (false_prefix, false_val) = self.inline_expr(false_val)?;
                prefix.extend(true_prefix);
                prefix.extend(false_prefix);
                Ok((
                    prefix,
                    Expr::Select {
                        cond: Box::new(cond),
                        true_val: Box::new(true_val),
                        false_val: Box::new(false_val),
                    },
                ))
            }
            Expr::Cast { target, value } => {
                let (prefix, value) = self.inline_expr(value)?;
                Ok((
                    prefix,
                    Expr::Cast {
                        target: target.clone(),
                        value: Box::new(value),
                    },
                ))
            }
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => {
                let (mut prefix, index) = self.inline_expr(index)?;
                let (expected_prefix, expected) = match expected.as_deref() {
                    Some(expected) => {
                        let (prefix, expected) = self.inline_expr(expected)?;
                        (prefix, Some(Box::new(expected)))
                    }
                    None => (Vec::new(), None),
                };
                let (value_prefix, value) = self.inline_expr(value)?;
                prefix.extend(expected_prefix);
                prefix.extend(value_prefix);
                Ok((
                    prefix,
                    Expr::Atomic {
                        op: *op,
                        buffer: buffer.clone(),
                        index: Box::new(index),
                        expected,
                        value: Box::new(value),
                        ordering: *ordering,
                    },
                ))
            }
        }
    }

    #[inline]
    pub(crate) fn inline_call(&mut self, op_id: &str, args: &[Expr]) -> Result<(Vec<Node>, Expr)> {
        if self.stack.iter().any(|active| active == op_id) {
            return Err(Error::InlineCycle {
                op_id: op_id.to_string(),
            });
        }

        let mut prefix = Vec::with_capacity(args.len());
        let mut inlined_args = Vec::with_capacity(args.len());
        for arg in args {
            let (arg_prefix, arg) = self.inline_expr(arg)?;
            prefix.extend(arg_prefix);
            inlined_args.push(arg);
        }

        let callee = (self.resolver)(op_id).ok_or_else(|| Error::InlineUnknownOp {
            op_id: op_id.to_string(),
        })?;
        self.stack.push(op_id.to_string());
        let result = self.expand_callee(op_id, &callee, inlined_args);
        self.stack.pop();
        let (callee_prefix, value) = result?;
        prefix.extend(callee_prefix);
        Ok((prefix, value))
    }

    #[inline]
    pub(crate) fn expand_callee(
        &mut self,
        op_id: &str,
        callee: &Program,
        args: Vec<Expr>,
    ) -> Result<(Vec<Node>, Expr)> {
        let call_id = self.next_call_id;
        self.next_call_id = self.next_call_id.saturating_add(1);
        let prefix = format!("_vyre_inl{call_id}_");
        let expected_args = input_buffers(callee).len();
        if args.len() != expected_args {
            return Err(Error::InlineArgCountMismatch {
                op_id: op_id.to_string(),
                expected: expected_args,
                got: args.len(),
            });
        }
        let output = output_buffer(op_id, callee)?;
        let result_name = format!("{prefix}result");
        let mut expander = CalleeExpander {
            ctx: self,
            prefix,
            vars: HashMap::default(),
            input_args: input_arg_map(callee, args),
            output_name: Ident::from(output.name()),
            result_name: result_name.clone(),
            saw_output: false,
        };

        let mut nodes = Vec::with_capacity(callee.entry().len() + 1);
        nodes.push(Node::let_bind(&result_name, zero_value(&output.element())));
        nodes.extend(expander.nodes(callee.entry())?);

        if !expander.saw_output {
            return Err(Error::InlineNoOutput {
                op_id: op_id.to_string(),
            });
        }

        Ok((nodes, Expr::var(&result_name)))
    }
}
