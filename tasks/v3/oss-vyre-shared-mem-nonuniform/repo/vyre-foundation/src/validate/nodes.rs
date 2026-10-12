use crate::ir_inner::model::expr::{Expr, Ident};
#[cfg(test)]
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::BufferDecl;
#[cfg(test)]
use crate::ir_inner::model::types::BufferAccess;
use crate::ir_inner::model::types::DataType;
#[cfg(test)]
use crate::validate::barrier;
#[cfg(test)]
use crate::validate::binding::check_sibling_duplicate;
#[cfg(test)]
use crate::validate::bytes_rejection;
#[cfg(test)]
use crate::validate::depth::{self, LimitState};
#[cfg(test)]
use crate::validate::expr_rules::validate_expr;
#[cfg(test)]
use crate::validate::shadowing;
#[cfg(test)]
use crate::validate::typecheck::expr_type;
#[cfg(test)]
use crate::validate::uniformity::is_uniform;
#[cfg(test)]
use crate::validate::ValidationOptions;
#[cfg(test)]
use crate::validate::ValidationReport;
use crate::validate::{err, Binding, ValidationError};
use rustc_hash::FxHashMap;
#[cfg(test)]
use rustc_hash::FxHashSet;

pub(crate) type ScopeLog = Vec<(Ident, Option<Binding>)>;

#[inline]
#[cfg(test)]
pub(crate) fn validate_nodes(
    nodes: &[Node],
    buffers: &FxHashMap<&str, &BufferDecl>,
    scope: &mut FxHashMap<Ident, Binding>,
    divergent: bool,
    depth: usize,
    limits: &mut LimitState,
    options: ValidationOptions<'_>,
    report: &mut ValidationReport,
) {
    let mut region_bindings = FxHashSet::default();
    validate_nodes_inner(
        nodes,
        buffers,
        scope,
        divergent,
        depth,
        limits,
        options,
        report,
        &mut region_bindings,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn validate_nodes_inner(
    nodes: &[Node],
    buffers: &FxHashMap<&str, &BufferDecl>,
    scope: &mut FxHashMap<Ident, Binding>,
    divergent: bool,
    depth: usize,
    limits: &mut LimitState,
    options: ValidationOptions<'_>,
    report: &mut ValidationReport,
    region_bindings: &mut FxHashSet<Ident>,
    mut scope_log: Option<&mut ScopeLog>,
) {
    for node in nodes {
        validate_node_inner(
            node,
            buffers,
            scope,
            divergent,
            depth,
            limits,
            options,
            report,
            region_bindings,
            scope_log.as_deref_mut(),
        );
    }

    if let Some(pos) = nodes.iter().position(|n| matches!(n, Node::Return)) {
        if pos != nodes.len().saturating_sub(1) {
            report.errors.push(err(
                "unreachable statements after `return`. Fix: remove statements after `return` or reorder them.".to_string(),
            ));
        }
    }
}

#[allow(clippy::too_many_lines, clippy::unnested_or_patterns)]
#[cfg(test)]
fn validate_node_inner(
    node: &Node,
    buffers: &FxHashMap<&str, &BufferDecl>,
    scope: &mut FxHashMap<Ident, Binding>,
    divergent: bool,
    depth: usize,
    limits: &mut LimitState,
    options: ValidationOptions<'_>,
    report: &mut ValidationReport,
    region_bindings: &mut FxHashSet<Ident>,
    scope_log: Option<&mut ScopeLog>,
) {
    depth::check_limits(limits, depth, &mut report.errors);

    match node {
        Node::Let { name, value } => {
            validate_expr(value, buffers, scope, options, report, 0);
            let duplicate_sibling = check_sibling_duplicate(
                name,
                region_bindings,
                options.allow_shadowing,
                &mut report.errors,
            );
            if !duplicate_sibling {
                shadowing::check_local(name, scope, options, &mut report.errors);
            }
            let ty_opt = expr_type(value, buffers, scope);
            let ty = ty_opt.clone().unwrap_or(DataType::U32);
            let ty_known = ty_opt.is_some();
            let uniform = is_uniform(value, scope);
            insert_binding(
                scope,
                name.clone(),
                Binding {
                    ty,
                    ty_known,
                    mutable: true,
                    uniform,
                },
                scope_log,
            );
        }
        Node::Assign { name, value } => {
            if let Some(binding) = scope.get(name.as_str()) {
                if !binding.mutable {
                    report.errors.push(err(format!(
                        "V011: assignment to loop variable `{name}`. Fix: loop variables are immutable."
                    )));
                }
                if binding.ty_known {
                    if let Some(value_ty) = expr_type(value, buffers, scope) {
                        if value_ty != binding.ty {
                            report.errors.push(err(format!(
                                "V045: assignment to `{name}` has type `{value_ty}` but the binding was declared as `{declared}`. Fix: cast the value to `{declared}` or introduce a new binding with the intended type.",
                                declared = binding.ty
                            )));
                        }
                    }
                }
            } else if let Some(buf) = buffers.get(name.as_str()) {
                if buf.access != BufferAccess::ReadWrite {
                    report.errors.push(err(format!(
                        "assignment to buffer `{name}` requires read-write storage but declared access is `{access:?}`. Fix: use a read-write/output buffer or store into a mutable local binding.",
                        access = buf.access
                    )));
                }
                if let Some(value_ty) = expr_type(value, buffers, scope) {
                    let elem = &buf.element;
                    let compatible = store_value_compatible(&value_ty, elem);
                    if !compatible {
                        report.errors.push(err(format!(
                            "V045: assignment to buffer `{name}` has type `{value_ty}` but the buffer element type is `{elem}`. Fix: cast the value to `{elem}` or write to a buffer with the intended element type."
                        )));
                    }
                }
            } else {
                report.errors.push(err(format!(
                    "assignment to undeclared variable `{name}`. Fix: add `let {name} = ...;` before this assignment."
                )));
            }
            validate_expr(value, buffers, scope, options, report, 0);
            // Reassignment with a divergent rhs taints the binding's
            // uniformity for the remainder of its lifetime.
            let new_uniform = is_uniform(value, scope);
            if let Some(binding) = scope.get_mut(name.as_str()) {
                binding.uniform = binding.uniform && new_uniform;
            }
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            bytes_rejection::check_store(buffer, buffers, &mut report.errors);
            if let Some(buf) = buffers.get(buffer.as_str()) {
                if let Some(val_ty) = expr_type(value, buffers, scope) {
                    let elem = &buf.element;
                    let compatible = store_value_compatible(&val_ty, elem);
                    if !compatible {
                        let legal_targets = store_value_targets(elem);
                        report.errors.push(err(format!(
                            "Node::Store buffer `{buffer}` value has type `{val_ty}` but element type is `{elem}`. Fix: cast/store using one of {}.", legal_targets
                        )));
                    }
                }
                if let Some(index_ty) = expr_type(index, buffers, scope) {
                    if index_ty != DataType::U32 {
                        report.errors.push(err(format!(
                            "Node::Store buffer `{buffer}` index has type `{index_ty}` but must be `u32`. Fix: cast the index to U32 before storing."
                        )));
                    }
                }
                check_constant_store_index(buffer, buf, index, &mut report.errors);
            }
            validate_expr(index, buffers, scope, options, report, 0);
            validate_expr(value, buffers, scope, options, report, 0);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            validate_expr(cond, buffers, scope, options, report, 0);
            if let Some(cond_ty) = expr_type(cond, buffers, scope) {
                if !matches!(cond_ty, DataType::U32 | DataType::Bool) {
                    report.errors.push(err(format!(
                        "Node::If condition has type `{cond_ty}` but must be `u32` or `bool`. Fix: cast or rewrite the condition expression to produce `u32` or `bool`."
                    )));
                }
            }
            // Branches stay non-divergent only when the parent scope is
            // already uniform AND the condition is uniform across the
            // workgroup. A non-uniform cond splits invocations across
            // the two branches; a divergent parent already failed the
            // uniformity precondition so we conservatively propagate.
            let branch_divergent = divergent || !is_uniform(cond, scope);
            validate_scoped_nested_nodes(
                then,
                buffers,
                scope,
                branch_divergent,
                depth,
                limits,
                options,
                report,
                |_, _| {},
            );
            validate_scoped_nested_nodes(
                otherwise,
                buffers,
                scope,
                branch_divergent,
                depth,
                limits,
                options,
                report,
                |_, _| {},
            );
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            validate_expr(from, buffers, scope, options, report, 0);
            validate_expr(to, buffers, scope, options, report, 0);
            if let Some(from_ty) = expr_type(from, buffers, scope) {
                if from_ty != DataType::U32 {
                    report.errors.push(err(format!(
                        "Node::Loop from-bound has type `{from_ty}`; legal loop bound type is `u32`. Fix: cast the `from` bound to `u32`."
                    )));
                }
            }
            if let Some(to_ty) = expr_type(to, buffers, scope) {
                if to_ty != DataType::U32 {
                    report.errors.push(err(format!(
                        "Node::Loop to-bound has type `{to_ty}`; legal loop bound type is `u32`. Fix: cast the `to` bound to `u32`."
                    )));
                }
            }
            shadowing::check_local(var, scope, options, &mut report.errors);
            // The loop body is divergent only when its parent already is
            // OR when either bound varies across the workgroup. Uniform
            // bounds keep every invocation in lockstep  -  same iteration
            // count, same loop-var value at each step  -  so a barrier
            // inside is reached by every lane simultaneously.
            let bounds_uniform = is_uniform(from, scope) && is_uniform(to, scope);
            let body_divergent = divergent || !bounds_uniform;
            // The loop counter inherits the bounds' uniformity; in a
            // uniform-bound loop every lane sees the same counter value
            // at the same source position.
            let var_uniform = bounds_uniform && !divergent;
            validate_scoped_nested_nodes(
                body,
                buffers,
                scope,
                body_divergent,
                depth,
                limits,
                options,
                report,
                |scope, scope_log| {
                    insert_binding(
                        scope,
                        var.clone(),
                        Binding {
                            ty: DataType::U32,
                            ty_known: true,
                            mutable: false,
                            uniform: var_uniform,
                        },
                        Some(scope_log),
                    );
                },
            );
        }
        Node::Return => {}
        Node::Block(nodes) => {
            validate_scoped_nested_nodes(
                nodes,
                buffers,
                scope,
                divergent,
                depth,
                limits,
                options,
                report,
                |_, _| {},
            );
        }
        Node::Barrier { ordering } => {
            barrier::check_barrier(divergent, *ordering, &mut report.errors);
        }
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => {
            if count_offset % 4 != 0 {
                report.errors.push(err(format!(
                    "indirect dispatch offset {count_offset} is not 4-byte aligned. Fix: use an offset aligned to a u32 dispatch count tuple."
                )));
            }
            if !buffers.contains_key(count_buffer.as_str()) {
                report.errors.push(err(format!(
                    "indirect dispatch references unknown buffer `{count_buffer}`. Fix: declare the count buffer before validation."
                )));
            }
        }
        Node::AsyncLoad { tag, .. } | Node::AsyncStore { tag, .. } | Node::AsyncWait { tag } => {
            if tag.is_empty() {
                report.errors.push(err(
                    "async stream tag is empty. Fix: use a stable non-empty tag to pair AsyncLoad and AsyncWait nodes."
                        .to_string(),
                ));
            }
        }
        Node::Trap { .. } | Node::Resume { .. } => {}
        Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
            validate_collective_support(options, &mut report.errors);
            validate_collective_buffer(buffer, buffers, &mut report.errors);
        }
        Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
            validate_collective_support(options, &mut report.errors);
            validate_collective_buffer(input, buffers, &mut report.errors);
            validate_collective_buffer(output, buffers, &mut report.errors);
            if let (Some(input), Some(output)) =
                (buffers.get(input.as_str()), buffers.get(output.as_str()))
            {
                if input.element != output.element {
                    report.errors.push(err(format!(
                        "V046: collective input/output element mismatch: `{}` is `{}`, `{}` is `{}`. Fix: use matching element types before collective lowering.",
                        input.name(),
                        input.element,
                        output.name(),
                        output.element
                    )));
                }
            }
        }
        Node::Region { body, .. } => {
            // Region is a tracing / grouping marker (emitted by
            // `crate::region::wrap_anonymous` / `wrap_child` so traces
            // and op-id breadcrumbs surface compositionally)  -  NOT a
            // variable-scoping construct. Builders all over vyre-libs
            // assume `Node::let_bind("acc", …)` declared at one
            // if_then level remains visible to a `wrap_child(...)`
            // sibling that reads `acc` (the gqa_attention max/sum/write
            // pass split, the python parser per-segment helpers, the
            // visual::gradient stop-blend chain, every Cat-A reduction
            // that wraps its hot loop in a named child region).
            //
            // Scoping here breaks those compositions: the inner
            // let_binds die on Region exit and downstream sibling
            // reads fail with V001/V032/V008. Dispatching through the
            // same recursive walker WITHOUT a fresh scope frame
            // restores the visible-across-siblings semantic the
            // builders depend on.
            //
            // True scoping constructs (Block, If/Else branches, Loop
            // body) keep their `validate_scoped_nested_nodes` call
            // sites  -  Region is the one that was misclassified.
            let mut region_bindings = FxHashSet::default();
            validate_nodes_inner(
                body,
                buffers,
                scope,
                divergent,
                depth.saturating_add(1),
                limits,
                options,
                report,
                &mut region_bindings,
                None,
            );
        }
        Node::Opaque(extension) => {
            if extension.extension_kind().is_empty() {
                report.errors.push(err(
                    "V031: opaque node extension has an empty extension_kind. Fix: return a stable non-empty namespace from NodeExtension::extension_kind.",
                ));
            }
            if extension.debug_identity().is_empty() {
                report.errors.push(err(format!(
                    "V031: opaque node extension `{}` has an empty debug_identity. Fix: return a stable human-readable identity from NodeExtension::debug_identity.",
                    extension.extension_kind()
                )));
            }
            if let Err(message) = extension.validate_extension() {
                report.errors.push(err(format!(
                    "V031: opaque node extension `{}`/`{}` failed validation: {message}",
                    extension.extension_kind(),
                    extension.debug_identity()
                )));
            }
        }
    }
}

#[cfg(test)]
fn validate_collective_support(options: ValidationOptions<'_>, errors: &mut Vec<ValidationError>) {
    if !options.supports_distributed_collectives() {
        errors.push(err(
            "V046: distributed collective nodes require backend collective support. Fix: validate with BackendCapabilities { supports_distributed_collectives: true, .. } or lower collectives before this backend.".to_string(),
        ));
    }
}

#[cfg(test)]

fn validate_collective_buffer(
    name: &Ident,
    buffers: &FxHashMap<&str, &BufferDecl>,
    errors: &mut Vec<ValidationError>,
) {
    let Some(buffer) = buffers.get(name.as_str()) else {
        errors.push(err(format!(
            "V046: collective references unknown buffer `{name}`. Fix: declare the collective buffer before validation."
        )));
        return;
    };
    if buffer.access == BufferAccess::Workgroup {
        errors.push(err(format!(
            "V046: collective buffer `{name}` is workgroup-local. Fix: use device/global storage visible to the distributed backend."
        )));
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn validate_scoped_nested_nodes(
    nodes: &[Node],
    buffers: &FxHashMap<&str, &BufferDecl>,
    scope: &mut FxHashMap<Ident, Binding>,
    divergent: bool,
    depth: usize,
    limits: &mut LimitState,
    options: ValidationOptions<'_>,
    report: &mut ValidationReport,
    configure_scope: impl FnOnce(&mut FxHashMap<Ident, Binding>, &mut ScopeLog),
) {
    let mut scope_log = Vec::new();
    let mut region_bindings = FxHashSet::default();
    configure_scope(scope, &mut scope_log);
    validate_nodes_inner(
        nodes,
        buffers,
        scope,
        divergent,
        depth.saturating_add(1),
        limits,
        options,
        report,
        &mut region_bindings,
        Some(&mut scope_log),
    );
    restore_scope(scope, scope_log);
}

pub(crate) fn check_constant_store_index(
    buffer_name: &str,
    buffer: &BufferDecl,
    index: &Expr,
    errors: &mut Vec<ValidationError>,
) {
    if buffer.count == 0 {
        return;
    }
    match index {
        Expr::LitU32(value) if *value >= buffer.count => {
            errors.push(err(format!(
                "V036: store index {value} overflows buffer `{buffer_name}` with count {}. Fix: keep constant store indices below the declared element count.",
                buffer.count
            )));
        }
        Expr::LitI32(value) if *value < 0 => {
            errors.push(err(format!(
                "V036: store index {value} overflows buffer `{buffer_name}` with count {}. Fix: keep constant store indices in 0..{}.",
                buffer.count,
                buffer.count
            )));
        }
        Expr::LitI32(value) => {
            let as_u32 = u32::try_from(*value).unwrap_or(u32::MAX);
            if as_u32 >= buffer.count {
                errors.push(err(format!(
                    "V036: store index {value} overflows buffer `{buffer_name}` with count {}. Fix: keep constant store indices below the declared element count.",
                    buffer.count
                )));
            }
        }
        _ => {}
    }
}

pub(crate) fn insert_binding(
    scope: &mut FxHashMap<Ident, Binding>,
    name: Ident,
    binding: Binding,
    scope_log: Option<&mut ScopeLog>,
) {
    let previous = scope.insert(name.clone(), binding);
    if let Some(scope_log) = scope_log {
        scope_log.push((name, previous));
    }
}

pub(crate) fn restore_scope(scope: &mut FxHashMap<Ident, Binding>, mut scope_log: ScopeLog) {
    while let Some((name, previous)) = scope_log.pop() {
        if let Some(binding) = previous {
            scope.insert(name, binding);
        } else {
            scope.remove(&name);
        }
    }
}

/// True if `value` and `element` are the SAME-WIDTH integer type differing only
/// in signedness (U32<->I32, U64<->I64).
///
/// A buffer element's signedness is observed only on LOAD (sign- vs zero-extend
/// on use); a STORE writes the raw little-endian word, so storing a U32-typed
/// value into an I32 buffer (or the 64-bit pair) is a bit-exact reinterpret —
/// exactly `i32_slot = u32_val as i32` in Rust. This is load-bearing because the
/// typechecker types Mod/bitwise/shift results as U32 regardless of operand
/// signedness (Add/Sub/Mul/Div preserve the operand type via Frame::Bin), so a
/// valid `store(i32_buffer, rem(i32, i32))` would otherwise be rejected. Every
/// lower layer already reinterprets: the naga emitter coerces the store value to
/// the element type (`coerce_value_to_type` -> `As{Sint/Uint, 4}`), PTX stores
/// are typeless `st.global.b32`, and the reference oracle stores the value's raw
/// bytes. NOT a widening/narrowing coercion (those change bits): only the
/// same-width signed/unsigned reinterpret is bit-preserving and allowed here.
#[inline]
#[must_use]
pub(crate) fn same_width_int_reinterpret(value: &DataType, element: &DataType) -> bool {
    matches!(
        (value, element),
        (DataType::U32, DataType::I32)
            | (DataType::I32, DataType::U32)
            | (DataType::U64, DataType::I64)
            | (DataType::I64, DataType::U64)
    )
}

/// The single source of truth for whether a value of type `value` may be written
/// into a buffer whose element type is `element` (by `Node::Store` OR by an
/// assignment to a buffer binding — both write the same raw word, so they MUST
/// agree; they previously diverged, with the assign path silently accepting
/// `Bool <-> U32` that `Node::Store` rejected).
///
/// Permitted beyond an exact match, all bit-preserving or backend-coerced writes:
///   * `U32 <-> Bytes` — a packed-byte buffer round-trip.
///   * `U32 <-> Bool` — a comparison/flag (0/1) stored into a u32 buffer (the
///     emitter coerces Bool via `x != 0` / `select(1u, 0u)`); the `U32 -> Bool`
///     direction is inert because a `bool` storage-buffer element is not
///     host-shareable in WGSL, but it is kept for symmetry with the assign path.
///   * `F32 -> F32` — identity (named explicitly so the `Bytes`/vector arms above
///     cannot be reached for a float).
///   * same-width signed/unsigned integer reinterpret (`U32<->I32`, `U64<->I64`).
/// A float-to-int, int-to-float, or differing-width write is NOT permitted (it
/// would change bits) and must use an explicit `Cast`.
#[inline]
#[must_use]
pub(crate) fn store_value_compatible(value: &DataType, element: &DataType) -> bool {
    value == element
        || matches!(
            (value, element),
            (DataType::U32, DataType::Bytes)
                | (DataType::Bytes, DataType::U32)
                | (DataType::U32, DataType::Bool)
                | (DataType::Bool, DataType::U32)
                | (DataType::F32, DataType::F32)
        )
        || same_width_int_reinterpret(value, element)
}

#[inline]
pub(crate) fn store_value_targets(element: &DataType) -> String {
    let mut targets = vec![element.clone()];
    let legal = match element {
        DataType::U32 => vec![DataType::Bytes, DataType::I32],
        DataType::Bytes => vec![DataType::U32],
        DataType::I32 => vec![DataType::U32],
        DataType::U64 => vec![DataType::I64],
        DataType::I64 => vec![DataType::U64],
        _ => Vec::new(),
    };
    for target in legal {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }

    targets
        .into_iter()
        .map(|target| format!("`{target}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Ident};

    #[test]
    fn store_value_targets_u32_includes_bytes_and_i32() {
        let targets = store_value_targets(&DataType::U32);
        assert!(
            targets.contains("u32"),
            "target list should contain u32: {targets}"
        );
        assert!(
            targets.contains("bytes"),
            "target list should contain bytes: {targets}"
        );
        // i32 is the same-width signed sibling: a bit-exact reinterpret store.
        assert!(
            targets.contains("i32"),
            "target list should contain i32 (same-width reinterpret): {targets}"
        );
        assert!(
            !targets.contains("bool"),
            "target list must not allow bool/u32 store coercion: {targets}"
        );
        assert!(
            !targets.contains("f32"),
            "target list must not allow f32/u32 store coercion (different bit semantics): {targets}"
        );
    }

    #[test]
    fn store_value_targets_f32_is_self_only() {
        let targets = store_value_targets(&DataType::F32);
        assert!(targets.contains("f32"));
        assert!(!targets.contains("u32"));
    }

    #[test]
    fn same_width_int_reinterpret_only_same_width_signedness() {
        // The bit-exact same-width pairs are allowed.
        assert!(same_width_int_reinterpret(&DataType::U32, &DataType::I32));
        assert!(same_width_int_reinterpret(&DataType::I32, &DataType::U32));
        assert!(same_width_int_reinterpret(&DataType::U64, &DataType::I64));
        assert!(same_width_int_reinterpret(&DataType::I64, &DataType::U64));
        // Different-width or non-integer pairs are NOT (they would change bits).
        assert!(!same_width_int_reinterpret(&DataType::U32, &DataType::U64));
        assert!(!same_width_int_reinterpret(&DataType::U32, &DataType::U8));
        assert!(!same_width_int_reinterpret(&DataType::F32, &DataType::U32));
        assert!(!same_width_int_reinterpret(&DataType::U32, &DataType::F32));
        assert!(!same_width_int_reinterpret(&DataType::U32, &DataType::U32));
    }

    #[test]
    fn store_value_compatible_unifies_store_and_assign_rules() {
        // Exact match, byte round-trip, bool flag, f32 identity, same-width int.
        assert!(store_value_compatible(&DataType::U32, &DataType::U32));
        assert!(store_value_compatible(&DataType::U32, &DataType::Bytes));
        assert!(store_value_compatible(&DataType::Bytes, &DataType::U32));
        assert!(store_value_compatible(&DataType::Bool, &DataType::U32));
        assert!(store_value_compatible(&DataType::U32, &DataType::Bool));
        assert!(store_value_compatible(&DataType::F32, &DataType::F32));
        assert!(store_value_compatible(&DataType::I32, &DataType::U32));
        assert!(store_value_compatible(&DataType::U64, &DataType::I64));
        // Bit-changing writes are NOT permitted (need an explicit Cast).
        assert!(!store_value_compatible(&DataType::F32, &DataType::U32));
        assert!(!store_value_compatible(&DataType::U32, &DataType::F32));
        assert!(!store_value_compatible(&DataType::Bool, &DataType::F32));
        assert!(!store_value_compatible(&DataType::U32, &DataType::U64));
        assert!(!store_value_compatible(&DataType::U8, &DataType::U32));
    }

    #[test]
    fn check_constant_store_index_within_bounds_no_error() {
        let buf = BufferDecl::read_write("buf", 0, DataType::U32).with_count(4);
        let mut errors = Vec::new();
        check_constant_store_index("buf", &buf, &Expr::u32(3), &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn check_constant_store_index_at_boundary_errors() {
        let buf = BufferDecl::read_write("buf", 0, DataType::U32).with_count(4);
        let mut errors = Vec::new();
        check_constant_store_index("buf", &buf, &Expr::u32(4), &mut errors);
        assert_eq!(
            errors.len(),
            1,
            "index == count should overflow: {errors:?}"
        );
    }

    #[test]
    fn check_constant_store_index_negative_i32_errors() {
        let buf = BufferDecl::read_write("buf", 0, DataType::U32).with_count(4);
        let mut errors = Vec::new();
        check_constant_store_index("buf", &buf, &Expr::i32(-1), &mut errors);
        assert_eq!(errors.len(), 1, "negative index should error: {errors:?}");
    }

    #[test]
    fn check_constant_store_index_zero_count_skips() {
        let buf = BufferDecl::read_write("buf", 0, DataType::U32);
        let mut errors = Vec::new();
        check_constant_store_index("buf", &buf, &Expr::u32(999), &mut errors);
        assert!(
            errors.is_empty(),
            "count=0 means dynamic and must be accepted"
        );
    }

    #[test]
    fn check_constant_store_index_dynamic_index_skips() {
        let buf = BufferDecl::read_write("buf", 0, DataType::U32).with_count(4);
        let mut errors = Vec::new();
        check_constant_store_index("buf", &buf, &Expr::Var(Ident::from("i")), &mut errors);
        assert!(errors.is_empty(), "dynamic index must be accepted");
    }
}
