//! Reverse-mode autodiff IR transform.
//!
//! The main entry point is [`grad`]: given a forward `Program`, a set of
//! output buffer names, and a set of input buffer names, it emits a new
//! `Program` whose stores write the gradients of the outputs w.r.t. the
//! inputs into `grad_<input>` buffers.

use rustc_hash::FxHashMap;

use crate::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program, UnOp};

use super::error::AutodiffError;
mod expr;
use expr::{emit_adjoint_expr, insert_pullback};

/// Per-forward-node pullback expression metadata.
///
/// Keys are stable per-transform pullback node ids in reverse-walk emission
/// order. Values are the adjoint expression consumed by that forward statement.
pub type PullbackMap = FxHashMap<usize, Expr>;

/// Compute reverse-mode gradients for a forward Program.
///
/// # Arguments
///
/// * `program`  -  the forward-pass Program.
/// * `outputs`  -  buffer names whose values to differentiate (the "loss").
/// * `inputs`  -  buffer names to compute gradients w.r.t. Gradient buffers
///   `grad_<name>` are added to the output Program.
///
/// # Returns
///
/// A new `Program` that:
/// 1. Re-declares all forward buffers (inputs as `ReadOnly`, outputs as `ReadOnly`).
/// 2. Declares fresh `grad_<input>` `ReadWrite` buffers for each input in `inputs`.
/// 3. Seeds `grad_<output> = 1.0` for each output in `outputs`.
/// 4. Walks the forward body in reverse, emitting adjoint accumulation stores.
///
/// # Errors
///
/// Returns `AutodiffError` if the Program contains non-differentiable ops in
/// the gradient path, or if an output/input buffer name is not found.
pub fn grad(
    program: &Program,
    outputs: &[&str],
    inputs: &[&str],
) -> Result<Program, AutodiffError> {
    grad_with_pullback(program, outputs, inputs).map(|(program, _pullbacks)| program)
}

/// Compute reverse-mode gradients and return top-level pullback metadata.
///
/// # Errors
///
/// Returns `AutodiffError` if the Program contains unsupported control flow,
/// non-differentiable expression nodes, or unknown input/output buffer names.
pub fn grad_with_pullback(
    program: &Program,
    outputs: &[&str],
    inputs: &[&str],
) -> Result<(Program, PullbackMap), AutodiffError> {
    validate_buffer_names(program, outputs, inputs)?;
    let (back_buffers, output_set, input_set, grad_source_set) =
        build_backward_buffers(program, outputs, inputs);

    // Build the backward body.
    let mut body: Vec<Node> = Vec::new();
    let i_expr = Expr::InvocationId { axis: 0 };
    let mut pullbacks = PullbackMap::default();
    let forward_nodes = program.entry();
    let mut adjoint_env: AdjointEnv = AdjointEnv::new(&grad_source_set, program);

    // Reverse-mode execution must declare every local adjoint before any
    // reverse contribution assigns to it. A previous implementation declared
    // `_adj_*` inside the reversed `Let` handler, which meant downstream
    // `Store` pullbacks could assign a local before declaration and then the
    // later `Let` reset it to zero. Predeclaring locals makes the generated IR
    // SSA-validator-friendly and preserves accumulated adjoints.
    let mut local_targets = Vec::new();
    collect_adjoint_targets(forward_nodes, &mut local_targets);
    for name in local_targets {
        let adj_name = adjoint_env.ensure_adjoint_var(name.as_str());
        body.push(Node::Let {
            name: adj_name.into(),
            value: Expr::f32(0.0),
        });
    }

    // Phase 0: Gradient buffers are read during accumulation. Make the
    // generated backward Program self-contained by clearing every declared
    // gradient lane before any seed or pullback store runs; callers must not
    // have to provide pre-zeroed scratch to get correct gradients.
    for source_name in &grad_source_set {
        let grad_name = format!("grad_{source_name}");
        body.push(Node::Store {
            buffer: grad_name.into(),
            index: i_expr.clone(),
            value: Expr::f32(0.0),
        });
    }

    // Phase 1: Seed  -  store 1.0 into each grad_<output>[i].
    for out_name in &output_set {
        let grad_name = format!("grad_{out_name}");
        body.push(Node::Store {
            buffer: grad_name.into(),
            index: i_expr.clone(),
            value: Expr::f32(1.0),
        });
    }

    // Phase 2: Reverse walk of forward body.
    // Collect the forward nodes, then process them in reverse order.
    let mut next_pullback_id = 0usize;
    for node in forward_nodes.iter().rev() {
        emit_adjoint_node(
            node,
            &mut body,
            &mut adjoint_env,
            &output_set,
            &mut pullbacks,
            &mut next_pullback_id,
        )?;
    }

    // Phase 3: Flush accumulated adjoints to grad_<input> buffers.
    for inp_name in &input_set {
        let grad_name = format!("grad_{inp_name}");
        if let Some(accum_var) = adjoint_env.get_accumulator(inp_name) {
            body.push(Node::Store {
                buffer: grad_name.into(),
                index: i_expr.clone(),
                value: Expr::Var(accum_var.into()),
            });
        }
    }

    Ok((
        Program::wrapped(back_buffers, program.workgroup_size(), body),
        pullbacks,
    ))
}

fn validate_buffer_names(
    program: &Program,
    outputs: &[&str],
    inputs: &[&str],
) -> Result<(), AutodiffError> {
    for name in outputs.iter().chain(inputs.iter()) {
        if program
            .buffers()
            .iter()
            .all(|buffer| buffer.name() != *name)
        {
            return Err(AutodiffError::BufferNotFound {
                name: (*name).to_string(),
            });
        }
    }
    Ok(())
}

fn build_backward_buffers(
    program: &Program,
    outputs: &[&str],
    inputs: &[&str],
) -> (Vec<BufferDecl>, Vec<String>, Vec<String>, Vec<String>) {
    let mut back_buffers = Vec::new();
    let mut next_binding = 0u32;
    for fwd_buf in program.buffers() {
        back_buffers.push(
            BufferDecl::storage(
                fwd_buf.name(),
                next_binding,
                BufferAccess::ReadOnly,
                fwd_buf.element(),
            )
            .with_count(fwd_buf.count()),
        );
        next_binding += 1;
    }

    let output_set: Vec<String> = outputs.iter().map(ToString::to_string).collect();
    let input_set: Vec<String> = inputs.iter().map(ToString::to_string).collect();
    let grad_source_set = grad_buffer_source_names(program, &output_set, &input_set);
    let mut grad_buf_binding = FxHashMap::default();
    for name in &grad_source_set {
        let grad_name = format!("grad_{name}");
        if grad_buf_binding.contains_key(&grad_name) {
            continue;
        }
        let Some(fwd_buf) = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == name.as_str())
        else {
            continue;
        };
        let mut grad_decl = BufferDecl::read_write(&grad_name, next_binding, DataType::F32)
            .with_count(fwd_buf.count());
        if output_set.iter().any(|candidate| candidate == name)
            || input_set.iter().any(|candidate| candidate == name)
        {
            grad_decl = grad_decl.with_pipeline_live_out(true);
        }
        back_buffers.push(grad_decl);
        grad_buf_binding.insert(grad_name, next_binding);
        next_binding += 1;
    }
    (back_buffers, output_set, input_set, grad_source_set)
}

fn grad_buffer_source_names(
    program: &Program,
    outputs: &[String],
    inputs: &[String],
) -> Vec<String> {
    let mut names = Vec::new();
    for name in outputs.iter().chain(inputs) {
        push_unique_string(&mut names, name.as_str());
    }
    for buffer in program.buffers() {
        if buffer.element() == DataType::F32 {
            push_unique_string(&mut names, buffer.name());
        }
    }
    names
}

fn push_unique_string(out: &mut Vec<String>, name: &str) {
    if !out.iter().any(|existing| existing == name) {
        out.push(name.to_string());
    }
}

/// Environment tracking adjoint accumulation for each variable / buffer load.
struct AdjointEnv {
    /// Maps variable name → current adjoint expression accumulator variable name.
    var_adjoints: FxHashMap<String, String>,
    /// Counter for generating fresh adjoint variable names.
    fresh_counter: u32,
    /// Buffers with declared gradient storage in the backward Program.
    grad_buffers: Vec<String>,
    /// Declared element type for every forward buffer.
    buffer_types: FxHashMap<String, DataType>,
    /// Inferred type for forward locals.
    var_types: FxHashMap<String, DataType>,
}

impl AdjointEnv {
    fn new(grad_buffers: &[String], program: &Program) -> Self {
        let mut env = Self {
            var_adjoints: FxHashMap::default(),
            fresh_counter: 0,
            grad_buffers: grad_buffers.to_vec(),
            buffer_types: program
                .buffers()
                .iter()
                .map(|buffer| (buffer.name().to_string(), buffer.element()))
                .collect(),
            var_types: FxHashMap::default(),
        };
        env.record_forward_types(program.entry());
        env
    }

    /// Get or create an accumulator variable for the adjoint of `var_name`.
    fn ensure_adjoint_var(&mut self, var_name: &str) -> String {
        if let Some(existing) = self.var_adjoints.get(var_name) {
            return existing.clone();
        }
        let adj_name = format!("_adj_{var_name}_{}", self.fresh_counter);
        self.fresh_counter += 1;
        self.var_adjoints
            .insert(var_name.to_string(), adj_name.clone());
        adj_name
    }

    /// Get the accumulator variable name for a buffer input, if one was created.
    fn get_accumulator(&self, buf_name: &str) -> Option<String> {
        self.var_adjoints.get(buf_name).cloned()
    }

    fn has_grad_buffer(&self, buf_name: &str) -> bool {
        self.grad_buffers.iter().any(|b| b == buf_name)
    }

    fn buffer_type(&self, buf_name: &str) -> Option<DataType> {
        self.buffer_types.get(buf_name).cloned()
    }

    fn record_forward_types(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Let { name, value } | Node::Assign { name, value } => {
                    if let Some(ty) = self.expr_type(value) {
                        self.var_types.insert(name.as_str().to_string(), ty);
                    } else {
                        self.var_types.remove(name.as_str());
                    }
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    self.record_forward_types(then);
                    self.record_forward_types(otherwise);
                }
                Node::Loop {
                    var,
                    body: loop_body,
                    ..
                } => {
                    self.var_types
                        .insert(var.as_str().to_string(), DataType::U32);
                    self.record_forward_types(loop_body);
                }
                Node::Block(body) => self.record_forward_types(body),
                Node::Region { body, .. } => self.record_forward_types(body),
                Node::Store { .. }
                | Node::IndirectDispatch { .. }
                | Node::AllReduce { .. }
                | Node::AllGather { .. }
                | Node::ReduceScatter { .. }
                | Node::Broadcast { .. }
                | Node::AsyncLoad { .. }
                | Node::AsyncStore { .. }
                | Node::AsyncWait { .. }
                | Node::Trap { .. }
                | Node::Resume { .. }
                | Node::Return
                | Node::Barrier { .. }
                | Node::Opaque(_) => {}
            }
        }
    }

    fn expr_type(&self, expr: &Expr) -> Option<DataType> {
        match expr {
            Expr::LitU32(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::Atomic { .. }
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. } => Some(DataType::U32),
            Expr::LitI32(_) => Some(DataType::I32),
            Expr::LitF32(_) => Some(DataType::F32),
            Expr::LitBool(_) => Some(DataType::Bool),
            Expr::Var(name) => self.var_types.get(name.as_str()).cloned(),
            Expr::Load { buffer, .. } => self.buffer_types.get(buffer.as_str()).cloned(),
            Expr::Cast { target, .. } => Some(target.clone()),
            Expr::BinOp { op, left, right } => self.binop_type(*op, left, right),
            Expr::UnOp { op, operand } => self.unop_type(op.clone(), operand),
            Expr::Select {
                true_val,
                false_val,
                ..
            } => {
                let true_ty = self.expr_type(true_val)?;
                let false_ty = self.expr_type(false_val)?;
                (true_ty == false_ty).then_some(true_ty)
            }
            Expr::Fma { a, b, c } => {
                let all_f32 = self.expr_type(a) == Some(DataType::F32)
                    && self.expr_type(b) == Some(DataType::F32)
                    && self.expr_type(c) == Some(DataType::F32);
                all_f32.then_some(DataType::F32)
            }
            Expr::Call { .. } => None,
            Expr::Opaque(extension) => extension.result_type(),
        }
    }

    fn binop_type(&self, op: BinOp, left: &Expr, right: &Expr) -> Option<DataType> {
        match op {
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::SaturatingAdd
            | BinOp::SaturatingSub
            | BinOp::SaturatingMul
            | BinOp::Min
            | BinOp::Max => {
                let left_ty = self.expr_type(left)?;
                let right_ty = self.expr_type(right)?;
                (left_ty == right_ty).then_some(left_ty)
            }
            BinOp::And
            | BinOp::Or
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Le
            | BinOp::Ge => Some(DataType::Bool),
            BinOp::Mod
            | BinOp::WrappingAdd
            | BinOp::WrappingSub
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Shl
            | BinOp::Shr
            | BinOp::AbsDiff
            | BinOp::Shuffle
            | BinOp::Ballot
            | BinOp::WaveReduce
            | BinOp::WaveBroadcast
            | BinOp::RotateLeft
            | BinOp::RotateRight
            | BinOp::MulHigh => Some(DataType::U32),
            BinOp::Opaque(_) => None,
            _ => None,
        }
    }

    fn unop_type(&self, op: UnOp, operand: &Expr) -> Option<DataType> {
        match op {
            UnOp::Negate
            | UnOp::BitNot
            | UnOp::Popcount
            | UnOp::Clz
            | UnOp::Ctz
            | UnOp::ReverseBits => self.expr_type(operand),
            UnOp::LogicalNot | UnOp::IsNan | UnOp::IsInf | UnOp::IsFinite => Some(DataType::Bool),
            UnOp::Cos
            | UnOp::Sin
            | UnOp::Abs
            | UnOp::Sqrt
            | UnOp::Floor
            | UnOp::Ceil
            | UnOp::Round
            | UnOp::Trunc
            | UnOp::Sign
            | UnOp::Exp
            | UnOp::Log
            | UnOp::Log2
            | UnOp::Exp2
            | UnOp::Tan
            | UnOp::Acos
            | UnOp::Asin
            | UnOp::Atan
            | UnOp::Tanh
            | UnOp::Sinh
            | UnOp::Cosh
            | UnOp::InverseSqrt
            | UnOp::Reciprocal => Some(DataType::F32),
            UnOp::Unpack4Low | UnOp::Unpack4High | UnOp::Unpack8Low | UnOp::Unpack8High => None,
            _ => None,
        }
    }
}

fn collect_adjoint_targets(nodes: &[Node], out: &mut Vec<Ident>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } | Node::Assign { name, .. } => push_unique_ident(out, name),
            Node::If {
                then, otherwise, ..
            } => {
                collect_adjoint_targets(then, out);
                collect_adjoint_targets(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => collect_adjoint_targets(body, out),
            Node::Region { body, .. } => collect_adjoint_targets(body, out),
            Node::Store { .. }
            | Node::IndirectDispatch { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Trap { .. }
            | Node::Resume { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::Opaque(_) => {}
        }
    }
}

fn push_unique_ident(out: &mut Vec<Ident>, name: &Ident) {
    if !out.iter().any(|existing| existing == name) {
        out.push(name.clone());
    }
}

/// Emit adjoint nodes for a single forward Node.
#[expect(
    clippy::too_many_lines,
    reason = "autodiff node lowering is an exhaustive IR-node dispatch table; splitting it would scatter unsupported-control-flow errors"
)]
fn emit_adjoint_node(
    node: &Node,
    body: &mut Vec<Node>,
    env: &mut AdjointEnv,
    output_set: &[String],
    pullbacks: &mut PullbackMap,
    next_pullback_id: &mut usize,
) -> Result<(), AutodiffError> {
    match node {
        // Forward: let x = value
        // Backward: propagate adjoint of x through value
        Node::Let { name, value } => {
            let var_name = name.as_str();
            let adj_var = env.ensure_adjoint_var(var_name);
            let adj_expr = Expr::Var(adj_var.into());
            insert_pullback(pullbacks, next_pullback_id, adj_expr.clone());
            // Propagate adjoint through the expression tree.
            emit_adjoint_expr(value, &adj_expr, body, env)?;
        }
        // Forward: store buf[idx] = value
        // Backward: adjoint of value comes from grad_buf[idx]
        Node::Store {
            buffer,
            index,
            value,
        } => {
            let buf_name = buffer.as_str();
            let has_grad_buffer =
                output_set.iter().any(|o| o == buf_name) || env.has_grad_buffer(buf_name);
            let grad_buf = format!("grad_{buf_name}");
            let adj_expr = if has_grad_buffer {
                Expr::Load {
                    buffer: grad_buf.clone().into(),
                    index: Box::new(index.clone()),
                }
            } else {
                Expr::f32(0.0)
            };
            insert_pullback(pullbacks, next_pullback_id, adj_expr.clone());
            emit_adjoint_expr(value, &adj_expr, body, env)?;
            if has_grad_buffer {
                body.push(Node::Store {
                    buffer: grad_buf.into(),
                    index: index.clone(),
                    value: Expr::f32(0.0),
                });
            }
        }
        // Forward: x = value (reassignment)
        // Same as Let for adjoint purposes.
        Node::Assign { name, value } => {
            let adj_var = env.ensure_adjoint_var(name.as_str());
            let adj_expr = Expr::Var(adj_var.into());
            insert_pullback(pullbacks, next_pullback_id, adj_expr.clone());
            emit_adjoint_expr(value, &adj_expr, body, env)?;
        }
        // Forward: if cond { then } else { otherwise }
        // Backward: route adjoint through the branch that was taken.
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let mut then_body = Vec::new();
            for n in then.iter().rev() {
                emit_adjoint_node(
                    n,
                    &mut then_body,
                    env,
                    output_set,
                    pullbacks,
                    next_pullback_id,
                )?;
            }
            let mut else_body = Vec::new();
            for n in otherwise.iter().rev() {
                emit_adjoint_node(
                    n,
                    &mut else_body,
                    env,
                    output_set,
                    pullbacks,
                    next_pullback_id,
                )?;
            }
            body.push(Node::If {
                cond: cond.clone(),
                then: then_body,
                otherwise: else_body,
            });
        }
        // Forward: for var in from..to { loop_body }
        // Backward: run the adjoint of loop_body in reverse iteration order.
        Node::Loop {
            var,
            from,
            to,
            body: loop_body,
        } => {
            let mut adj_body = Vec::new();
            for n in loop_body.iter().rev() {
                emit_adjoint_node(
                    n,
                    &mut adj_body,
                    env,
                    output_set,
                    pullbacks,
                    next_pullback_id,
                )?;
            }
            // Reverse iteration: for var in from..to, but with the
            // induction variable remapped to the reversed index so that
            // adj_body[0] corresponds to the *last* forward iteration.
            //
            // reversed_var = (to - 1) - (var - from)
            //              = to - 1 - var + from
            //
            // We keep the loop bounds as from..to (same iteration count)
            // and substitute every occurrence of the original `var` in
            // adj_body with the reversed expression so that the body
            // addresses elements in reverse order at runtime.
            let reversed_index = Expr::sub(
                Expr::sub(to.clone(), Expr::u32(1)),
                Expr::sub(Expr::var(var.as_str()), from.clone()),
            );
            let adj_body =
                crate::transform::subst::substitute_nodes(&adj_body, var, &reversed_index);
            body.push(Node::Loop {
                var: var.clone(),
                from: from.clone(),
                to: to.clone(),
                body: adj_body,
            });
        }
        // Barrier  -  pass through.
        Node::Barrier { ordering } => {
            body.push(Node::barrier_with_ordering(*ordering));
        }
        // Block  -  unwrap and recurse.
        Node::Block(nodes) => {
            for n in nodes.iter().rev() {
                emit_adjoint_node(n, body, env, output_set, pullbacks, next_pullback_id)?;
            }
        }
        // Region  -  recurse into body.
        Node::Region {
            generator,
            source_region,
            body: region_body,
        } => {
            let mut adj_region_body = Vec::new();
            for n in region_body.iter().rev() {
                emit_adjoint_node(
                    n,
                    &mut adj_region_body,
                    env,
                    output_set,
                    pullbacks,
                    next_pullback_id,
                )?;
            }
            body.push(Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: std::sync::Arc::new(adj_region_body),
            });
        }
        // Return, IndirectDispatch, Async*, Trap, Resume  -  not differentiable control flow.
        Node::Return
        | Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. } => {
            return Err(AutodiffError::UnsupportedNode {
                kind: format!("{node:?}").chars().take(60).collect(),
            });
        }
        // Opaque  -  cannot differentiate unknown ops.
        Node::Opaque(_) => {
            return Err(AutodiffError::UnsupportedNode {
                kind: "Node::Opaque".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
