use super::super::CalleeExpander;
use crate::error::Result;
use crate::ir::{Expr, Ident, Node};

impl CalleeExpander<'_> {
    #[inline]
    pub(crate) fn nodes(&mut self, nodes: &[Node]) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for node in nodes {
            out.extend(self.node(node)?);
        }
        Ok(out)
    }

    #[inline]
    pub(crate) fn node(&mut self, node: &Node) -> Result<Vec<Node>> {
        match node {
            Node::Let { name, value } => self.bind(name, value),
            Node::Assign { name, value } => self.assign(name, value),
            Node::Store {
                buffer,
                index,
                value,
            } => self.store(buffer, index, value),
            Node::If {
                cond,
                then,
                otherwise,
            } => self.branch(cond, then, otherwise),
            Node::Loop {
                var,
                from,
                to,
                body,
            } => self.loop_for(var, from, to, body),
            Node::Return => Ok(vec![Node::Return]),
            Node::Block(nodes) => Ok(vec![Node::Block(self.nodes(nodes)?)]),
            Node::Barrier { ordering } => Ok(vec![Node::barrier_with_ordering(*ordering)]),
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => Ok(vec![Node::IndirectDispatch {
                count_buffer: count_buffer.clone(),
                count_offset: *count_offset,
            }]),
            // Async offset/size are expressions that can reference callee-local
            // variables; they must be alpha-renamed (and any nested call
            // hoisted) exactly like a Store index, or a callee-local in the
            // offset dangles against its renamed `let` declaration.
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                let (mut prefix, offset) = self.expr(offset)?;
                let (size_prefix, size) = self.expr(size)?;
                prefix.extend(size_prefix);
                prefix.push(Node::async_load_ext(
                    source.clone(),
                    destination.clone(),
                    offset,
                    size,
                    tag.clone(),
                ));
                Ok(prefix)
            }
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                let (mut prefix, offset) = self.expr(offset)?;
                let (size_prefix, size) = self.expr(size)?;
                prefix.extend(size_prefix);
                prefix.push(Node::async_store(
                    source.clone(),
                    destination.clone(),
                    offset,
                    size,
                    tag.clone(),
                ));
                Ok(prefix)
            }
            Node::AsyncWait { tag } => Ok(vec![Node::async_wait(tag)]),
            // A trap address is an expression that can reference a callee-local;
            // rename it like any other expression position.
            Node::Trap { address, tag } => {
                let (mut prefix, address) = self.expr(address)?;
                prefix.push(Node::Trap {
                    address: Box::new(address),
                    tag: tag.clone(),
                });
                Ok(prefix)
            }
            // Resume carries only an async tag; the collectives reference only
            // global buffer names. Neither holds a callee-local value
            // expression, so cloning verbatim preserves correctness.
            Node::Resume { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. } => Ok(vec![node.clone()]),
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_nodes = std::sync::Arc::try_unwrap(body.clone()).unwrap_or_else(|arc| (*arc).clone());
                Ok(vec![Node::Region {
                    generator: generator.clone(),
                    source_region: source_region.clone(),
                    body: std::sync::Arc::new(self.nodes(&body_nodes)?),
                }])
            }
            Node::Opaque(extension) => Err(crate::error::Error::lowering(format!(
                "inliner cannot expand opaque statement extension `{}`/`{}`. Fix: lower the extension to core Node variants before inlining.",
                extension.extension_kind(),
                extension.debug_identity()
            ))),
        }
    }

    #[inline]
    pub(crate) fn bind(&mut self, name: &Ident, value: &Expr) -> Result<Vec<Node>> {
        let renamed = self.rename_decl(name);
        let (mut prefix, value) = self.expr(value)?;
        prefix.push(Node::let_bind(&renamed, value));
        Ok(prefix)
    }

    #[inline]
    pub(crate) fn assign(&mut self, name: &Ident, value: &Expr) -> Result<Vec<Node>> {
        let (mut prefix, value) = self.expr(value)?;
        prefix.push(Node::assign(self.rename_use(name), value));
        Ok(prefix)
    }

    #[inline]
    pub(crate) fn store(
        &mut self,
        buffer: &Ident,
        index: &Expr,
        value: &Expr,
    ) -> Result<Vec<Node>> {
        let (mut prefix, index) = self.expr(index)?;
        let (value_prefix, value) = self.expr(value)?;
        prefix.extend(value_prefix);
        if self.output_name == *buffer {
            self.saw_output = true;
            prefix.push(Node::assign(&self.result_name, value));
        } else {
            prefix.push(Node::store(buffer, index, value));
        }
        Ok(prefix)
    }

    #[inline]
    pub(crate) fn branch(
        &mut self,
        cond: &Expr,
        then: &[Node],
        otherwise: &[Node],
    ) -> Result<Vec<Node>> {
        let (mut prefix, cond) = self.expr(cond)?;
        prefix.push(Node::if_then_else(
            cond,
            self.nodes(then)?,
            self.nodes(otherwise)?,
        ));
        Ok(prefix)
    }

    #[inline]
    pub(crate) fn loop_for(
        &mut self,
        var: &Ident,
        from: &Expr,
        to: &Expr,
        body: &[Node],
    ) -> Result<Vec<Node>> {
        let renamed = self.rename_decl(var);
        let (mut prefix, from) = self.expr(from)?;
        let (to_prefix, to) = self.expr(to)?;
        prefix.extend(to_prefix);
        prefix.push(Node::loop_for(&renamed, from, to, self.nodes(body)?));
        Ok(prefix)
    }
}
