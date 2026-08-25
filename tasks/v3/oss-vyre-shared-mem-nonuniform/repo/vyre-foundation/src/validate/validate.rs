#![allow(clippy::unwrap_used)]
//! Top-level validation entry point.
//!
//! This module runs the complete validation pipeline on a `Program`:
//! buffer declarations, node structure, expression types, depth limits,
//! and output markers. Every error is returned as a `ValidationError`
//! with an actionable `Fix:` hint.

pub use super::depth::{
    DEFAULT_MAX_CALL_DEPTH, DEFAULT_MAX_EXPR_DEPTH, DEFAULT_MAX_NESTING_DEPTH,
    DEFAULT_MAX_NODE_COUNT,
};
use super::expr_rules::validate_output_markers;
use super::fusion_safety::{collect_expr_accesses, NodeAccesses};
// Self-composition (duplicate self-exclusive regions) is enforced in
// `PreorderValidator::run` via `self_comp_counts`  -  do not add a second
// `duplicate_self_exclusive_regions` walk here.
use super::{depth, err, nodes, ValidationError, ValidationOptions, ValidationReport};
use crate::composition::self_exclusive_region_key;
use crate::ir_inner::model::expr::{Expr, Ident};
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;
use crate::ir_inner::model::types::{BufferAccess, DataType};
use crate::visit::traits::{dispatch_node, NodeVisitor};
use hashbrown::hash_map::RawEntryMut;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::convert::Infallible;
use std::ops::ControlFlow;

/// Validate a program for structural and semantic correctness.
///
/// The validator checks the stable rules documented in
/// `vyre/docs/ir/validation.md`: workgroup dimensions must be positive,
/// buffer names and bindings must be unique, workgroup buffers must have
/// a positive element count, and the node tree must respect depth limits.
/// A successful validation (empty error vector) means the program is
/// safe to lower to any backend.
///
/// # Examples
///
/// ```
/// use vyre::ir::{Program, validate};
///
/// let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
/// let errors = validate(&program);
/// assert!(errors.is_empty());
/// ```
#[inline]
#[must_use]
pub fn validate(program: &Program) -> Vec<ValidationError> {
    validate_with_options(program, ValidationOptions::default()).errors
}

/// Report ONLY the `Fma`-operand f32 violations (rule `V028`) in `program`.
///
/// This is the focused subset that emit backends must run before lowering.
/// An `Fma` node with non-f32 operands is unique among IR-validity hazards in
/// that it BOTH silently miscompiles (integer operands lower to `a*b+c`, not
/// fused-multiply-add) AND emits successfully — no downstream stage rejects it.
/// Every other validation rule corresponds to a program that either emits
/// correctly or fails with a dedicated, more-specific downstream diagnostic, so
/// emit boundaries must not run full [`validate`] (it would preempt those
/// messages and trip rules unrelated to silent miscompilation).
///
/// Reuses the full validator so the `Fma` type inference stays single-sourced
/// with [`validate`]; the `V028` filter is pinned by `fma_f32_violations_*`
/// tests in this module so a message change cannot silently disable it.
#[must_use]
pub fn fma_f32_violations(program: &Program) -> Vec<ValidationError> {
    validate(program)
        .into_iter()
        .filter(|error| error.message().starts_with("V028:"))
        .collect()
}

/// Validate a program with explicit backend/shadowing options.
///
/// `ValidationOptions::default()` performs best-effort universal validation:
/// it enforces backend-independent structural rules but does not reject
/// backend-specific cast targets unless a concrete backend capability contract
/// is supplied.
#[inline]
#[must_use]
pub fn validate_with_options(
    program: &Program,
    options: ValidationOptions<'_>,
) -> ValidationReport {
    let mut report = ValidationReport {
        errors: Vec::with_capacity(program.buffers().len() + program.entry().len()),
        warnings: Vec::new(),
    };

    if let Some(message) = program.top_level_region_violation() {
        report.errors.push(err(message));
    }

    for (axis, &size) in program.workgroup_size.iter().enumerate() {
        if size == 0 {
            report.errors.push(err(format!(
                "workgroup_size[{axis}] is 0. Fix: all workgroup dimensions must be >= 1."
            )));
        }
    }

    let mut seen_names = FxHashSet::default();
    seen_names.reserve(program.buffers().len());
    let mut seen_bindings = FxHashSet::default();
    seen_bindings.reserve(program.buffers().len());
    for buf in program.buffers() {
        if !seen_names.insert(&buf.name) {
            report.errors.push(err(format!(
                "duplicate buffer name `{}`. Fix: each buffer must have a unique name.",
                buf.name
            )));
        }
        if buf.access != BufferAccess::Workgroup && !seen_bindings.insert(buf.binding) {
            report.errors.push(err(format!(
                "duplicate binding slot {} (buffer `{}`). Fix: each buffer must have a unique binding.",
                buf.binding, buf.name
            )));
        }
        if buf.access == BufferAccess::Workgroup && buf.count == 0 {
            report.errors.push(err(format!(
                "workgroup buffer `{}` has count 0. Fix: declare a positive element count.",
                buf.name
            )));
        }
        validate_output_buffer_element_type(buf, &mut report.errors);
    }
    validate_output_markers(program.buffers(), &mut report.errors);

    let mut buffer_map: FxHashMap<&str, &crate::ir_inner::model::program::BufferDecl> =
        FxHashMap::default();
    buffer_map.reserve(program.buffers().len());
    buffer_map.extend(program.buffers().iter().map(|b| (b.name.as_ref(), b)));

    let mut validator = PreorderValidator::new(options, buffer_map);
    validator.run(program.entry());
    report.errors.append(&mut validator.errors);
    report.warnings.append(&mut validator.warnings);

    // P-1.0-V2.2: linear-type discipline checker. Reports buffers
    // whose `LinearType` declaration is violated by the actual usage
    // count in the IR.
    report
        .errors
        .extend(crate::validate::linear_type::check_linear_types(program));

    // P-1.0-V3.2: shape-predicate refinement checker. Reports buffers
    // whose static `count` violates the declared `ShapePredicate`.
    report
        .errors
        .extend(crate::validate::shape_predicate::check_shape_predicates(
            program,
        ));

    report
}

fn validate_output_buffer_element_type(
    buf: &crate::ir_inner::model::program::BufferDecl,
    errors: &mut Vec<ValidationError>,
) {
    if !buf.is_output() {
        return;
    }

    if matches!(buf.element(), DataType::Array { .. } | DataType::Tensor) {
        errors.push(err(format!(
            "output buffer `{}` uses unsupported element type `{}`. Fix: output buffers must use fixed-width scalar or vector element types, not Array or Tensor.",
            buf.name(),
            buf.element()
        )));
    }
}

// ------------------------------------------------------------------
// PreorderValidator  -  single-pass explicit-stack traversal
// ------------------------------------------------------------------

use super::barrier;
use super::binding::{check_sibling_duplicate, Binding};
use super::bytes_rejection;
use super::expr_rules;
use super::shadowing;
use super::typecheck::expr_type;
use super::uniformity::is_uniform;
// use super::report::warn;

/// Scope frame pushed for every nested node sequence.
struct ScopeFrame<'p> {
    scope_log: nodes::ScopeLog,
    region_bindings: FxHashSet<Ident>,
    divergent: bool,
    depth: usize,
    nodes: &'p [Node],
}

/// Stack frames for the explicit traversal.
enum Frame<'p> {
    /// Visit a single node (pre-order).
    Child(&'p Node),
    /// Post-order action for `If`: extend parent alias state with cond accesses.
    PostIf,
    /// Post-order action for `Loop`: extend parent alias state with from/to accesses.
    PostLoop,
    /// Enter a new scope.
    PushScope {
        divergent: bool,
        depth: usize,
        nodes: &'p [Node],
    },
    /// Leave the current scope and check `Return` position.
    PopScope,
    /// Enter a fresh alias tracking frame.
    PushAlias,
    /// Restore the parent alias tracking frame.
    PopAlias,
    /// Inject the loop variable binding into the current scope. The
    /// `uniform` flag mirrors the loop's bound uniformity: in a
    /// uniform-bound loop every invocation walks the same iteration
    /// count with the same counter value, so the loop var is itself
    /// uniform.
    InsertLoopVar { var: Ident, uniform: bool },
}

/// Single-pass validator that performs all node-tree checks in one
/// explicit-stack traversal.
struct PreorderValidator<'p, 'o> {
    options: ValidationOptions<'o>,
    buffers: FxHashMap<&'p str, &'p crate::ir_inner::model::program::BufferDecl>,
    scope: FxHashMap<Ident, Binding>,
    scope_stack: SmallVec<[ScopeFrame<'p>; 16]>,
    limits: depth::LimitState,
    alias_reads: FxHashSet<Ident>,
    alias_atomics: FxHashSet<Ident>,
    alias_stack: SmallVec<[(FxHashSet<Ident>, FxHashSet<Ident>); 8]>,
    pending_alias_extensions: SmallVec<[NodeAccesses; 8]>,
    self_comp_counts: hashbrown::HashMap<String, usize>,
    errors: Vec<ValidationError>,
    warnings: Vec<super::ValidationWarning>,
    /// HOT PATH (`PreorderValidator::validate_expr`): reuse one report buffer per expression so we do not allocate fresh error/warning vectors for every `validate_expr` invocation while traversing the IR tree.
    expr_report_scratch: ValidationReport,
}

impl<'p, 'o> PreorderValidator<'p, 'o> {
    fn new(
        options: ValidationOptions<'o>,
        buffers: FxHashMap<&'p str, &'p crate::ir_inner::model::program::BufferDecl>,
    ) -> Self {
        Self {
            options,
            buffers,
            scope: FxHashMap::default(),
            scope_stack: SmallVec::new(),
            limits: depth::LimitState::default(),
            alias_reads: FxHashSet::default(),
            alias_atomics: FxHashSet::default(),
            alias_stack: SmallVec::new(),
            pending_alias_extensions: SmallVec::new(),
            self_comp_counts: hashbrown::HashMap::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
            expr_report_scratch: ValidationReport::default(),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "single-pass validator run loop is an explicit stack machine; keeping frames together preserves stack-safety and validation-order invariants"
    )]
    fn run(&mut self, nodes: &'p [Node]) {
        let mut stack: SmallVec<[Frame<'p>; 128]> = SmallVec::new();
        stack.push(Frame::PopScope);
        for node in nodes.iter().rev() {
            stack.push(Frame::Child(node));
        }
        stack.push(Frame::PushAlias);
        stack.push(Frame::PushScope {
            divergent: false,
            depth: 0,
            nodes,
        });

        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Child(node) => {
                    if dispatch_node(self, node).is_break() {
                        break;
                    }
                    match node {
                        Node::If {
                            cond,
                            then,
                            otherwise,
                            ..
                        } => {
                            let depth = self.current_depth();
                            // Branches stay non-divergent only when the
                            // parent scope is already uniform AND the
                            // condition is uniform across the workgroup.
                            // A non-uniform cond splits invocations
                            // across the two branches, so any barrier
                            // inside is reached by only some lanes.
                            let parent_divergent = self.current_divergent();
                            let branch_divergent =
                                parent_divergent || !is_uniform(cond, &self.scope);
                            stack.push(Frame::PostIf);
                            push_nested_sequence(
                                &mut stack,
                                otherwise,
                                branch_divergent,
                                depth + 1,
                                None,
                            );
                            push_nested_sequence(
                                &mut stack,
                                then,
                                branch_divergent,
                                depth + 1,
                                None,
                            );
                        }
                        Node::Loop {
                            var,
                            from,
                            to,
                            body,
                        } => {
                            let depth = self.current_depth();
                            // The loop body is divergent only when its
                            // parent already is OR when either bound
                            // varies across the workgroup. Uniform
                            // bounds keep every invocation in lockstep
                            //  -  same iteration count, same loop-var
                            // value at each step  -  so a barrier inside
                            // is reached by every lane simultaneously.
                            let parent_divergent = self.current_divergent();
                            let bounds_uniform =
                                is_uniform(from, &self.scope) && is_uniform(to, &self.scope);
                            let body_divergent = parent_divergent || !bounds_uniform;
                            // Loop var inherits the bounds' uniformity
                            // when the parent is also uniform; if the
                            // parent is divergent the var only matters
                            // within already-divergent context.
                            let var_uniform = bounds_uniform && !parent_divergent;
                            stack.push(Frame::PostLoop);
                            push_nested_sequence(
                                &mut stack,
                                body,
                                body_divergent,
                                depth + 1,
                                Some(Frame::InsertLoopVar {
                                    var: var.clone(),
                                    uniform: var_uniform,
                                }),
                            );
                        }
                        Node::Block(body) => {
                            let depth = self.current_depth();
                            let divergent = self.current_divergent();
                            push_nested_sequence(&mut stack, body, divergent, depth + 1, None);
                        }
                        Node::Region { body, .. } => {
                            let depth = self.current_depth();
                            let divergent = self.current_divergent();
                            push_nested_sequence(&mut stack, body, divergent, depth + 1, None);
                        }
                        _ => {}
                    }
                }
                Frame::PostIf | Frame::PostLoop => {
                    if let Some(accesses) = self.pending_alias_extensions.pop() {
                        self.extend_alias(&accesses);
                    }
                }
                Frame::PushScope {
                    divergent,
                    depth,
                    nodes,
                } => {
                    self.scope_stack.push(ScopeFrame {
                        scope_log: Vec::new(),
                        region_bindings: FxHashSet::default(),
                        divergent,
                        depth,
                        nodes,
                    });
                }
                Frame::PopScope => {
                    let Some(frame) = self.scope_stack.pop() else {
                        self.errors.push(err(
                            "malformed validation frame stream: PopScope without matching PushScope. Fix: rebuild the program through the structured IR builder before validation.".to_string(),
                        ));
                        continue;
                    };
                    nodes::restore_scope(&mut self.scope, frame.scope_log);
                    if let Some(pos) = frame.nodes.iter().position(|n| matches!(n, Node::Return)) {
                        if pos != frame.nodes.len().saturating_sub(1) {
                            self.errors.push(err(
                                "unreachable statements after `return`. Fix: remove statements after `return` or reorder them.".to_string(),
                            ));
                        }
                    }
                }
                Frame::PushAlias => {
                    let reads = std::mem::take(&mut self.alias_reads);
                    let atomics = std::mem::take(&mut self.alias_atomics);
                    self.alias_stack.push((reads, atomics));
                    self.alias_reads = FxHashSet::default();
                    self.alias_atomics = FxHashSet::default();
                }
                Frame::PopAlias => {
                    let Some((reads, atomics)) = self.alias_stack.pop() else {
                        self.errors.push(err(
                            "malformed validation frame stream: PopAlias without matching PushAlias. Fix: rebuild the program through the structured IR builder before validation.".to_string(),
                        ));
                        continue;
                    };
                    let _ = std::mem::take(&mut self.alias_reads);
                    let _ = std::mem::take(&mut self.alias_atomics);
                    self.alias_reads = reads;
                    self.alias_atomics = atomics;
                }
                Frame::InsertLoopVar { var, uniform } => {
                    let Some(frame) = self.scope_stack.last_mut() else {
                        self.errors.push(err(format!(
                            "malformed validation frame stream: loop variable `{var}` inserted outside any scope. Fix: rebuild the program through the structured IR builder before validation."
                        )));
                        continue;
                    };
                    nodes::insert_binding(
                        &mut self.scope,
                        var.clone(),
                        Binding {
                            ty: DataType::U32,
                            ty_known: true,
                            mutable: false,
                            uniform,
                        },
                        Some(&mut frame.scope_log),
                    );
                }
            }
        }

        // Emit self-composition errors deterministically.
        let mut duplicates: Vec<String> = self
            .self_comp_counts
            .drain()
            .filter_map(|(generator, count)| (count > 1).then_some(generator))
            .collect();
        duplicates.sort_unstable();
        for generator in duplicates {
            self.errors.push(err(format!(
                "region `{generator}` is marked non-composable with itself but appears multiple times in one fused program. Fix: split the parser into separate dispatches, or give each instance distinct scratch storage before fusion."
            )));
        }
    }

    #[inline]
    fn current_divergent(&self) -> bool {
        self.scope_stack.last().is_some_and(|f| f.divergent)
    }

    #[inline]
    fn current_depth(&self) -> usize {
        self.scope_stack.last().map_or(0, |f| f.depth)
    }

    /// Run the legacy `validate_expr` helper and merge its diagnostics.
    fn validate_expr(&mut self, expr: &Expr, depth_level: usize) {
        self.expr_report_scratch.errors.clear();
        self.expr_report_scratch.warnings.clear();
        expr_rules::validate_expr(
            expr,
            &self.buffers,
            &self.scope,
            self.options,
            &mut self.expr_report_scratch,
            depth_level,
        );
        self.errors.append(&mut self.expr_report_scratch.errors);
        self.warnings.append(&mut self.expr_report_scratch.warnings);
    }

    fn validate_collective_buffer(&mut self, name: &Ident) {
        let Some(buffer) = self.buffers.get(name.as_str()) else {
            self.errors.push(err(format!(
                "V046: collective references unknown buffer `{name}`. Fix: declare the collective buffer before validation."
            )));
            return;
        };
        if buffer.access == BufferAccess::Workgroup {
            self.errors.push(err(format!(
                "V046: collective buffer `{name}` is workgroup-local. Fix: use device/global storage visible to the distributed backend."
            )));
        }
    }

    /// Report fusion-alias hazards between `accesses` and the current linear state.
    fn report_alias_hazards(&mut self, accesses: &NodeAccesses) {
        let mut hazards = accesses
            .atomic_buffers
            .intersection(&self.alias_reads)
            .cloned()
            .collect::<SmallVec<[Ident; 8]>>();
        hazards.extend(
            accesses
                .read_buffers
                .intersection(&self.alias_atomics)
                .cloned(),
        );
        hazards.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
        hazards.dedup();

        for buffer in hazards {
            self.errors.push(err(format!(
                "fusion hazard on buffer `{buffer}`: one node reads it non-atomically while another issues an atomic access without an explicit barrier. Fix: insert `Node::barrier()` between the read path and the atomic path, or rename the buffers before fusion."
            )));
        }
    }

    /// Extend the current alias frame with `accesses`.
    fn extend_alias(&mut self, accesses: &NodeAccesses) {
        self.alias_reads
            .extend(accesses.read_buffers.iter().cloned());
        self.alias_atomics
            .extend(accesses.atomic_buffers.iter().cloned());
    }
}

/// Push the stack frames needed to process a nested node sequence.
fn push_nested_sequence<'p>(
    stack: &mut SmallVec<[Frame<'p>; 128]>,
    nodes: &'p [Node],
    divergent: bool,
    depth: usize,
    pre_children: Option<Frame<'p>>,
) {
    stack.push(Frame::PopScope);
    stack.push(Frame::PopAlias);
    for child in nodes.iter().rev() {
        stack.push(Frame::Child(child));
    }
    if let Some(pre) = pre_children {
        stack.push(pre);
    }
    stack.push(Frame::PushAlias);
    stack.push(Frame::PushScope {
        divergent,
        depth,
        nodes,
    });
}

// ------------------------------------------------------------------
// NodeVisitor implementation
// ------------------------------------------------------------------

impl NodeVisitor for PreorderValidator<'_, '_> {
    type Break = Infallible;

    fn visit_let(&mut self, _node: &Node, name: &Ident, value: &Expr) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        self.validate_expr(value, 0);

        let Some(frame) = self.scope_stack.last_mut() else {
            self.errors.push(err(format!(
                "malformed validation frame stream: let binding `{name}` appeared outside any scope. Fix: rebuild the program through the structured IR builder before validation."
            )));
            return ControlFlow::Continue(());
        };
        // Same-region duplicate Lets are always invalid, even when
        // shadowing is allowed for nested scopes  -  the V032 contract
        // covered by `sibling_duplicate_lets_are_rejected_even_when_shadowing_is_allowed`.
        // `allow_shadowing` only opens nested scopes; siblings collide
        // unconditionally.
        let duplicate_sibling = check_sibling_duplicate(
            name,
            &mut frame.region_bindings,
            /*allow_duplicate_siblings=*/ false,
            &mut self.errors,
        );
        if !duplicate_sibling {
            shadowing::check_local(name, &self.scope, self.options, &mut self.errors);
        }
        let ty_opt = expr_type(value, &self.buffers, &self.scope);
        let ty = ty_opt.clone().unwrap_or(DataType::U32);
        let ty_known = ty_opt.is_some();
        let uniform = is_uniform(value, &self.scope);
        nodes::insert_binding(
            &mut self.scope,
            name.clone(),
            Binding {
                ty,
                ty_known,
                mutable: true,
                uniform,
            },
            Some(&mut frame.scope_log),
        );

        let mut accesses = NodeAccesses::default();
        collect_expr_accesses(value, &mut accesses);
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_assign(
        &mut self,
        _node: &Node,
        name: &Ident,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if let Some(binding) = self.scope.get(name.as_str()) {
            if !binding.mutable {
                self.errors.push(err(format!(
                    "V011: assignment to loop variable `{name}`. Fix: loop variables are immutable."
                )));
            }
            if binding.ty_known {
                if let Some(value_ty) = expr_type(value, &self.buffers, &self.scope) {
                    if value_ty != binding.ty {
                        self.errors.push(err(format!(
                            "V045: assignment to `{name}` has type `{value_ty}` but the binding was declared as `{declared}`. Fix: cast the value to `{declared}` or introduce a new binding with the intended type.",
                            declared = binding.ty
                        )));
                    }
                }
            }
        } else if let Some(buffer) = self.buffers.get(name.as_str()) {
            if buffer.access != BufferAccess::ReadWrite {
                self.errors.push(err(format!(
                    "assignment to buffer `{name}` requires read-write storage but declared access is `{access:?}`. Fix: use a read-write/output buffer or store into a mutable local binding.",
                    access = buffer.access
                )));
            }
            if let Some(value_ty) = expr_type(value, &self.buffers, &self.scope) {
                let elem = &buffer.element;
                let compatible = nodes::store_value_compatible(&value_ty, elem);
                if !compatible {
                    self.errors.push(err(format!(
                        "V045: assignment to buffer `{name}` has type `{value_ty}` but the buffer element type is `{elem}`. Fix: cast the value to `{elem}` or write to a buffer with the intended element type."
                    )));
                }
            }
        } else {
            self.errors.push(err(format!(
                "assignment to undeclared variable `{name}`. Fix: add `let {name} = ...;` before this assignment."
            )));
        }
        self.validate_expr(value, 0);

        // Reassigning with a divergent rhs taints the binding's
        // uniformity for the remainder of its lifetime.
        let new_uniform = is_uniform(value, &self.scope);
        if let Some(binding) = self.scope.get_mut(name.as_str()) {
            binding.uniform = binding.uniform && new_uniform;
        }

        let mut accesses = NodeAccesses::default();
        collect_expr_accesses(value, &mut accesses);
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_store(
        &mut self,
        _node: &Node,
        buffer: &Ident,
        index: &Expr,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        bytes_rejection::check_store(buffer, &self.buffers, &mut self.errors);
        if let Some(buf) = self.buffers.get(buffer.as_str()) {
            if let Some(val_ty) = expr_type(value, &self.buffers, &self.scope) {
                let elem = &buf.element;
                let compatible = nodes::store_value_compatible(&val_ty, elem);
                if !compatible {
                    let legal_targets = nodes::store_value_targets(elem);
                    self.errors.push(err(format!(
                        "Node::Store buffer `{buffer}` value has type `{val_ty}` but element type is `{elem}`. Fix: cast/store using one of {legal_targets}."
                    )));
                }
            }
            if let Some(index_ty) = expr_type(index, &self.buffers, &self.scope) {
                if index_ty != DataType::U32 {
                    self.errors.push(err(format!(
                        "Node::Store buffer `{buffer}` index has type `{index_ty}` but must be `u32`. Fix: cast the index to U32 before storing."
                    )));
                }
            }
            nodes::check_constant_store_index(buffer, buf, index, &mut self.errors);
        }
        self.validate_expr(index, 0);
        self.validate_expr(value, 0);

        let mut accesses = NodeAccesses::default();
        accesses.read_buffers.insert(buffer.clone());
        collect_expr_accesses(index, &mut accesses);
        collect_expr_accesses(value, &mut accesses);
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_if(
        &mut self,
        _node: &Node,
        cond: &Expr,
        _then: &[Node],
        _otherwise: &[Node],
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        self.validate_expr(cond, 0);
        if let Some(cond_ty) = expr_type(cond, &self.buffers, &self.scope) {
            if !matches!(cond_ty, DataType::U32 | DataType::Bool) {
                self.errors.push(err(format!(
                    "Node::If condition has type `{cond_ty}` but must be `u32` or `bool`. Fix: cast or rewrite the condition expression to produce `u32` or `bool`."
                )));
            }
        }

        let mut accesses = NodeAccesses::default();
        collect_expr_accesses(cond, &mut accesses);
        self.report_alias_hazards(&accesses);
        self.pending_alias_extensions.push(accesses);

        ControlFlow::Continue(())
    }

    fn visit_loop(
        &mut self,
        _node: &Node,
        var: &Ident,
        from: &Expr,
        to: &Expr,
        _body: &[Node],
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        self.validate_expr(from, 0);
        self.validate_expr(to, 0);
        if let Some(from_ty) = expr_type(from, &self.buffers, &self.scope) {
            if from_ty != DataType::U32 {
                self.errors.push(err(format!(
                    "Node::Loop from-bound has type `{from_ty}`; legal loop bound type is `u32`. Fix: cast the `from` bound to `u32`."
                )));
            }
        }
        if let Some(to_ty) = expr_type(to, &self.buffers, &self.scope) {
            if to_ty != DataType::U32 {
                self.errors.push(err(format!(
                    "Node::Loop to-bound has type `{to_ty}`; legal loop bound type is `u32`. Fix: cast the `to` bound to `u32`."
                )));
            }
        }
        shadowing::check_local(var, &self.scope, self.options, &mut self.errors);

        let mut accesses = NodeAccesses::default();
        collect_expr_accesses(from, &mut accesses);
        collect_expr_accesses(to, &mut accesses);
        self.report_alias_hazards(&accesses);
        self.pending_alias_extensions.push(accesses);

        ControlFlow::Continue(())
    }

    fn visit_indirect_dispatch(
        &mut self,
        _node: &Node,
        count_buffer: &Ident,
        count_offset: u64,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if count_offset % 4 != 0 {
            self.errors.push(err(format!(
                "indirect dispatch offset {count_offset} is not 4-byte aligned. Fix: use an offset aligned to a u32 dispatch count tuple."
            )));
        }
        if !self.buffers.contains_key(count_buffer.as_str()) {
            self.errors.push(err(format!(
                "indirect dispatch references unknown buffer `{count_buffer}`. Fix: declare the count buffer before validation."
            )));
        }

        let mut accesses = NodeAccesses::default();
        accesses.read_buffers.insert(count_buffer.clone());
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_async_load(
        &mut self,
        _node: &Node,
        source: &Ident,
        destination: &Ident,
        _offset: &Expr,
        _size: &Expr,
        tag: &Ident,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if tag.is_empty() {
            self.errors.push(err(
                "async stream tag is empty. Fix: use a stable non-empty tag to pair AsyncLoad and AsyncWait nodes."
                    .to_string(),
            ));
        }

        let mut accesses = NodeAccesses::default();
        accesses.read_buffers.insert(source.clone());
        accesses.read_buffers.insert(destination.clone());
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_async_store(
        &mut self,
        _node: &Node,
        source: &Ident,
        destination: &Ident,
        _offset: &Expr,
        _size: &Expr,
        tag: &Ident,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if tag.is_empty() {
            self.errors.push(err(
                "async stream tag is empty. Fix: use a stable non-empty tag to pair AsyncLoad and AsyncWait nodes."
                    .to_string(),
            ));
        }

        let mut accesses = NodeAccesses::default();
        accesses.read_buffers.insert(source.clone());
        accesses.read_buffers.insert(destination.clone());
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);

        ControlFlow::Continue(())
    }

    fn visit_async_wait(&mut self, _node: &Node, tag: &Ident) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if tag.is_empty() {
            self.errors.push(err(
                "async stream tag is empty. Fix: use a stable non-empty tag to pair AsyncLoad and AsyncWait nodes."
                    .to_string(),
            ));
        }
        ControlFlow::Continue(())
    }

    fn visit_trap(
        &mut self,
        _node: &Node,
        _address: &Expr,
        _tag: &Ident,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        ControlFlow::Continue(())
    }

    fn visit_resume(&mut self, _node: &Node, _tag: &Ident) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        ControlFlow::Continue(())
    }

    fn visit_return(&mut self, _node: &Node) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        ControlFlow::Continue(())
    }

    fn visit_barrier(&mut self, node: &Node) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        let divergent = self.current_divergent();
        let Node::Barrier { ordering } = node else {
            self.errors.push(err(
                "malformed barrier visitor dispatch. Fix: rebuild the program through the structured IR builder before validation."
                    .to_string(),
            ));
            return ControlFlow::Continue(());
        };
        barrier::check_barrier(divergent, *ordering, &mut self.errors);
        self.alias_reads.clear();
        self.alias_atomics.clear();
        ControlFlow::Continue(())
    }

    fn visit_collective(&mut self, node: &Node) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if !self.options.supports_distributed_collectives() {
            self.errors.push(err(
                "V046: distributed collective nodes require backend collective support. Fix: validate with BackendCapabilities { supports_distributed_collectives: true, .. } or lower collectives before this backend."
                    .to_string(),
            ));
        }

        let mut accesses = NodeAccesses::default();
        match node {
            Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
                self.validate_collective_buffer(buffer);
                accesses.read_buffers.insert(buffer.clone());
            }
            Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
                self.validate_collective_buffer(input);
                self.validate_collective_buffer(output);
                if let (Some(input_buf), Some(output_buf)) = (
                    self.buffers.get(input.as_str()),
                    self.buffers.get(output.as_str()),
                ) {
                    if input_buf.element != output_buf.element {
                        self.errors.push(err(format!(
                            "V046: collective input/output element mismatch: `{}` is `{}`, `{}` is `{}`. Fix: use matching element types before collective lowering.",
                            input_buf.name(),
                            input_buf.element,
                            output_buf.name(),
                            output_buf.element
                        )));
                    }
                }
                accesses.read_buffers.insert(input.clone());
                accesses.read_buffers.insert(output.clone());
            }
            _ => {}
        }
        self.report_alias_hazards(&accesses);
        self.extend_alias(&accesses);
        ControlFlow::Continue(())
    }

    fn visit_block(&mut self, _node: &Node, _body: &[Node]) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        ControlFlow::Continue(())
    }

    fn visit_region(
        &mut self,
        _node: &Node,
        generator: &Ident,
        _source_region: &Option<crate::ir_inner::model::expr::GeneratorRef>,
        _body: &[Node],
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if let Some(base) = self_exclusive_region_key(generator.as_str()) {
            match self.self_comp_counts.raw_entry_mut().from_key(base) {
                RawEntryMut::Occupied(mut o) => *o.get_mut() += 1,
                RawEntryMut::Vacant(v) => {
                    v.insert(base.to_string(), 1);
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _node: &Node,
        extension: &dyn crate::ir_inner::model::node::NodeExtension,
    ) -> ControlFlow<Self::Break> {
        let depth = self.current_depth();
        depth::check_limits(&mut self.limits, depth, &mut self.errors);
        if extension.extension_kind().is_empty() {
            self.errors.push(err(
                "V031: opaque node extension has an empty extension_kind. Fix: return a stable non-empty namespace from NodeExtension::extension_kind.",
            ));
        }
        if extension.debug_identity().is_empty() {
            self.errors.push(err(format!(
                "V031: opaque node extension `{}` has an empty debug_identity. Fix: return a stable human-readable identity from NodeExtension::debug_identity.",
                extension.extension_kind()
            )));
        }
        if let Err(message) = extension.validate_extension() {
            self.errors.push(err(format!(
                "V031: opaque node extension `{}`/`{}` failed validation: {message}",
                extension.extension_kind(),
                extension.debug_identity()
            )));
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]

mod tests {
    include!("validate_tests.rs");
}
