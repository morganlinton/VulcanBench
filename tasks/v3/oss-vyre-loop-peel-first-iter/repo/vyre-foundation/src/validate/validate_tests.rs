// Tests for `validate.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.

use super::*;
use crate::ir::{AtomicOp, BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use crate::validate::fusion_safety::validate_fusion_alias_hazards;
use crate::validate::self_composition::validate_self_composition;
use crate::MemoryOrdering;
use proptest::prelude::*;

// ------------------------------------------------------------------
// Legacy multi-walk validator (copied from pre-refactor code) for
// regression testing.
// ------------------------------------------------------------------
fn validate_with_options_legacy(
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
    let mut seen_bindings = FxHashSet::default();
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

    let mut scope = FxHashMap::default();
    let mut limits = depth::LimitState::default();
    nodes::validate_nodes(
        program.entry(),
        &buffer_map,
        &mut scope,
        false,
        0,
        &mut limits,
        options,
        &mut report,
    );
    validate_fusion_alias_hazards(program.entry(), &mut report.errors);
    validate_self_composition(program.entry(), &mut report.errors);

    report
}

// ------------------------------------------------------------------
// Proptest generators (adapted from transform::visit tests).
// ------------------------------------------------------------------
fn arb_ident() -> BoxedStrategy<String> {
    prop::sample::select(&["x", "y", "idx", "i", "acc"][..])
        .prop_map(str::to_string)
        .boxed()
}

fn arb_buffer_name() -> BoxedStrategy<String> {
    prop::sample::select(&["out", "input", "rw", "counts", "scratch"][..])
        .prop_map(str::to_string)
        .boxed()
}

fn arb_call_op() -> BoxedStrategy<String> {
    prop::sample::select(
        &[
            "test.noop",
            "test.add.u32",
            "test.mul.f32",
            "test.unknown_op",
        ][..],
    )
    .prop_map(str::to_string)
    .boxed()
}

fn arb_expr() -> BoxedStrategy<Expr> {
    let leaf = prop_oneof![
        any::<u32>().prop_map(Expr::LitU32),
        any::<i32>().prop_map(Expr::LitI32),
        any::<bool>().prop_map(Expr::LitBool),
        arb_ident().prop_map(Expr::var),
        arb_buffer_name().prop_map(Expr::buf_len),
        // Expr::Call with no arguments: exercises the validate_call code path
        // (previously absent, so single_pass_validator_matches_legacy never
        // covered the silent-fallback defect).
        arb_call_op().prop_map(|op| Expr::call(op, vec![])),
    ];

    leaf.prop_recursive(3, 48, 3, |inner| {
        prop_oneof![
            (arb_buffer_name(), inner.clone()).prop_map(|(buffer, index)| Expr::Load {
                buffer: buffer.into(),
                index: Box::new(index),
            }),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(left),
                right: Box::new(right),
            }),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::BinOp {
                op: BinOp::Sub,
                left: Box::new(left),
                right: Box::new(right),
            }),
            inner.clone().prop_map(|operand| Expr::UnOp {
                op: UnOp::Negate,
                operand: Box::new(operand),
            }),
            (inner.clone(), inner.clone(), inner.clone()).prop_map(
                |(cond, true_val, false_val)| Expr::Select {
                    cond: Box::new(cond),
                    true_val: Box::new(true_val),
                    false_val: Box::new(false_val),
                }
            ),
            inner.clone().prop_map(|value| Expr::Cast {
                target: DataType::U32,
                value: Box::new(value),
            }),
            (
                arb_buffer_name(),
                inner.clone(),
                proptest::option::of(inner.clone()),
                inner.clone(),
            )
                .prop_map(|(buffer, index, expected, value)| Expr::Atomic {
                    op: AtomicOp::Add,
                    buffer: buffer.into(),
                    index: Box::new(index),
                    expected: expected.map(Box::new),
                    value: Box::new(value),
                    ordering: MemoryOrdering::SeqCst,
                }),
        ]
    })
    .boxed()
}

fn arb_node() -> BoxedStrategy<Node> {
    arb_node_with_depth(3)
}

fn arb_node_with_depth(depth: u32) -> BoxedStrategy<Node> {
    let leaf = prop_oneof![
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Let {
            name: name.into(),
            value,
        }),
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Assign {
            name: name.into(),
            value,
        }),
        (arb_buffer_name(), arb_expr(), arb_expr()).prop_map(|(buffer, index, value)| {
            Node::Store {
                buffer: buffer.into(),
                index,
                value,
            }
        }),
        Just(Node::Return),
        Just(Node::barrier()),
    ];

    if depth == 0 {
        return leaf.boxed();
    }

    leaf.prop_recursive(2, 32, 2, move |inner| {
        prop_oneof![
            (
                arb_expr(),
                prop::collection::vec(inner.clone(), 0..=3),
                prop::collection::vec(inner.clone(), 0..=3),
            )
                .prop_map(|(cond, then, otherwise)| Node::If {
                    cond,
                    then,
                    otherwise,
                }),
            (
                arb_ident(),
                arb_expr(),
                arb_expr(),
                prop::collection::vec(inner.clone(), 0..=3),
            )
                .prop_map(|(var, from, to, body)| Node::Loop {
                    var: var.into(),
                    from,
                    to,
                    body,
                }),
            prop::collection::vec(inner, 0..=3).prop_map(Node::Block),
        ]
    })
    .boxed()
}

fn arb_program() -> BoxedStrategy<Program> {
    prop::collection::vec(arb_node(), 0..=8)
        .prop_map(|entry| {
            Program::wrapped(
                vec![
                    BufferDecl::output("out", 0, DataType::U32)
                        .with_count(8)
                        .with_output_byte_range(0..16),
                    BufferDecl::read("input", 1, DataType::U32).with_count(8),
                    BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
                    BufferDecl::read("counts", 3, DataType::U32).with_count(8),
                    BufferDecl::workgroup("scratch", 4, DataType::U32),
                ],
                [1, 1, 1],
                entry,
            )
        })
        .boxed()
}

// ------------------------------------------------------------------
// Regression test: new single-pass validator must emit exactly the
// same errors (+ warnings) as the old four-walk validator.
// ------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        ..ProptestConfig::default()
    })]

    #[test]
    fn single_pass_validator_matches_legacy(program in arb_program()) {
        let legacy = validate_with_options_legacy(&program, ValidationOptions::default());
        let modern = validate_with_options(&program, ValidationOptions::default());

        // Deterministic ordering: sort both error sets by message.
        let mut legacy_errors = legacy.errors;
        let mut modern_errors = modern.errors;
        legacy_errors.sort_by(|a, b| a.message.cmp(&b.message));
        modern_errors.sort_by(|a, b| a.message.cmp(&b.message));

        prop_assert_eq!(
            legacy_errors, modern_errors,
            "error mismatch between legacy and single-pass validator"
        );

        let mut legacy_warnings = legacy.warnings;
        let mut modern_warnings = modern.warnings;
        legacy_warnings.sort_by(|a, b| a.message.cmp(&b.message));
        modern_warnings.sort_by(|a, b| a.message.cmp(&b.message));

        prop_assert_eq!(
            legacy_warnings, modern_warnings,
            "warning mismatch between legacy and single-pass validator"
        );
    }
}

// ------------------------------------------------------------------
// F2 regression: let-binding a call result must not fabricate a U32
// type and fire false V045 on later assignments of a different type.
// ------------------------------------------------------------------

/// When `expr_type` returns `None` for `Expr::Call` (no dialect lookup provided),
/// `visit_let` previously recorded `DataType::U32` as the binding type. A later
/// assignment of `1.0f32` would then trigger a false V045 ("U32 expected, got F32").
///
/// After the fix the binding is recorded as `ty_known = false`, and V045 is
/// skipped — the program must validate without V045.
#[test]
fn call_result_binding_unknown_type_does_not_produce_false_v045() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![
            // let x = some_call() — type is unknown because no lookup is registered.
            Node::Let {
                name: "x".into(),
                value: Expr::Call {
                    op_id: "unknown.dialect.op".into(),
                    args: vec![],
                },
            },
            // assign x = 1.0f32 — valid if x is F32, but previously caused a false
            // V045 because x was recorded as U32 (the fabricated sentinel).
            Node::Assign {
                name: "x".into(),
                value: Expr::LitF32(1.0),
            },
            Node::Store {
                buffer: "out".into(),
                index: Expr::u32(0),
                value: Expr::var("x"),
            },
        ],
    );
    let report = validate_with_options(&program, ValidationOptions::default());
    // The only error must be V016 (no lookup for the call), NOT V045.
    let v045: Vec<_> = report
        .errors
        .iter()
        .filter(|e| e.message().contains("V045"))
        .collect();
    assert!(
        v045.is_empty(),
        "false V045 fired on call-result binding with unknown type: {:?}",
        v045
    );
    // Confirm that V016 IS emitted (the call itself is still rejected).
    assert!(
        report.errors.iter().any(|e| e.message().contains("V016")),
        "expected V016 for call with no lookup, got: {:?}",
        report.errors
    );
}

// ------------------------------------------------------------------
// `fma_f32_violations` — the focused subset emit backends run before
// lowering. Pins the `V028` filter so a message change cannot silently
// disable the integer-Fma rejection (which would re-open the Law-10
// silent `a*b+c` miscompile), and proves the filter excludes unrelated
// validation errors so emit boundaries don't preempt downstream
// diagnostics.
// ------------------------------------------------------------------
#[test]
fn fma_f32_violations_flags_integer_fma_with_actionable_message() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind(
            "bad_fma",
            Expr::Fma {
                a: Box::new(Expr::u32(1)),
                b: Box::new(Expr::u32(2)),
                c: Box::new(Expr::u32(3)),
            },
        )],
    );
    let violations = fma_f32_violations(&program);
    assert_eq!(
        violations.len(),
        3,
        "every non-f32 Fma operand (a, b, c) must be reported, got: {violations:?}"
    );
    for violation in &violations {
        assert!(
            violation.message().starts_with("V028:"),
            "fma_f32_violations must only return V028 errors, got: {}",
            violation.message()
        );
        assert!(
            violation.message().contains("Fma requires three f32 operands")
                && violation.message().contains("must be `f32`")
                && violation.message().contains("Fix:"),
            "V028 message must name the f32 contract and a fix, got: {}",
            violation.message()
        );
    }
}

#[test]
fn fma_f32_violations_empty_for_all_f32_operands() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::Fma {
                a: Box::new(Expr::LitF32(2.0)),
                b: Box::new(Expr::LitF32(3.0)),
                c: Box::new(Expr::LitF32(4.0)),
            },
        )],
    );
    assert!(
        fma_f32_violations(&program).is_empty(),
        "f32 Fma is valid and must not be flagged"
    );
}

#[test]
fn fma_f32_violations_ignores_unrelated_validation_errors() {
    // A program with NO Fma but a genuine validation error (zero workgroup
    // dimension). `validate` reports it; `fma_f32_violations` must NOT, so
    // emit boundaries calling this never preempt the dedicated diagnostic.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [0, 1, 1],
        Vec::new(),
    );
    assert!(
        !validate(&program).is_empty(),
        "zero workgroup dimension must be a validation error (guards the test premise)"
    );
    assert!(
        fma_f32_violations(&program).is_empty(),
        "non-Fma validation errors must be filtered out by fma_f32_violations"
    );
}

// ------------------------------------------------------------------
// Unpack UnOp recognition. `Unpack4/8 Low/High` previously fell through
// `validate_unop_operand`'s `_` catch-all and were rejected as "not
// recognized" — even though that message lists them as valid and every
// backend lowers them. Validate must recognize them and check the
// integer-word operand contract instead.
// ------------------------------------------------------------------
#[test]
fn validate_recognizes_integer_unpack_ops() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::UnOp {
                op: UnOp::Unpack8High,
                operand: Box::new(Expr::u32(0xDEAD_BEEF)),
            },
        )],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("is not recognized")),
        "integer unpack op must be recognized, got: {errors:?}"
    );
    assert!(
        !errors.iter().any(|e| e.message().contains("unpack ops require")),
        "a u32 operand is valid for unpack ops, got: {errors:?}"
    );
}

#[test]
fn validate_rejects_non_integer_unpack_operand_on_type_not_existence() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::UnOp {
                op: UnOp::Unpack4Low,
                operand: Box::new(Expr::LitF32(1.5)),
            },
        )],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("unpack ops require a 32-bit integer")
            && e.message().contains("Fix:")),
        "f32 unpack operand must be rejected with the integer-word contract, got: {errors:?}"
    );
    assert!(
        !errors.iter().any(|e| e.message().contains("is not recognized")),
        "unpack op must be rejected on operand type, not treated as unrecognized, got: {errors:?}"
    );
}

// ------------------------------------------------------------------
// Same-width integer store coercion (U32 <-> I32, U64 <-> I64).
//
// The typechecker types Mod / bitwise / shift results as U32 regardless of
// operand signedness (typecheck.rs `_ => DataType::U32`), while Add/Sub/Mul/Div
// preserve the operand type via Frame::Bin. A buffer element only distinguishes
// signedness when LOADED (sign- vs zero-extend on use); a STORE writes the raw
// 32/64-bit word, so storing a U32-typed value into an I32 buffer (or vice
// versa) is a bit-exact reinterpret. The naga emitter already coerces the store
// value to the element type (coerce_value_to_type -> As{Sint/Uint}), PTX stores
// are typeless `st.global.b32`, and the reference oracle stores the value's
// bytes — so every lower layer is byte-correct. The validator was the lone
// over-strict layer: it rejected `store(i32_buffer, rem(i32, i32))` (a valid,
// common signed-remainder store) with V045. These pin the coercion.
// ------------------------------------------------------------------
#[test]
fn store_signed_remainder_into_i32_buffer_validates() {
    // rem(i32, i32) is U32-typed but carries the SIGNED remainder bits; storing
    // it into an I32 buffer is a same-width reinterpret. Before the coercion this
    // was wrongly rejected V045 (the documented Mod-result-type gap).
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::I32).with_count(4),
            BufferDecl::read("a", 1, DataType::I32).with_count(4),
            BufferDecl::read("b", 2, DataType::I32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V045")
            || e.message().contains("value has type")),
        "store of a same-width int (rem result, u32-typed) into an i32 buffer must \
         validate (bit-exact reinterpret), got: {errors:?}"
    );
}

#[test]
fn store_signed_div_into_u32_buffer_validates() {
    // The reverse direction: div(i32, i32) is I32-typed (Frame::Bin preserves the
    // operand type); storing it into a U32 buffer is the same same-width
    // reinterpret and must also validate.
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(4),
            BufferDecl::read("a", 1, DataType::I32).with_count(4),
            BufferDecl::read("b", 2, DataType::I32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V045")
            || e.message().contains("value has type")),
        "store of an i32-typed value into a u32 buffer must validate, got: {errors:?}"
    );
}

#[test]
fn store_float_into_int_buffer_still_rejected() {
    // The coercion is ONLY same-width INTEGER reinterpret. A float value into an
    // i32 buffer is a real type error (different bit semantics, needs an explicit
    // cast) and must STILL be rejected — proving the coercion did not over-broaden.
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::I32).with_count(4),
            BufferDecl::read("f", 1, DataType::F32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("f", Expr::u32(0)),
        )],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e.message().contains("Node::Store")
            && e.message().contains("element type")),
        "storing an f32 value into an i32 buffer must still be rejected (no int/float \
         coercion), got: {errors:?}"
    );
}

#[test]
fn assign_signed_remainder_to_i32_buffer_validates() {
    // The buffer-ASSIGN path (visit_assign) must apply the same same-width int
    // reinterpret coercion as Node::Store — otherwise `buf = rem(i32, i32)` is
    // rejected while the equivalent store is allowed (an inconsistency between
    // two writes of the same logical value).
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("buf", 0, DataType::I32).with_count(4),
            BufferDecl::read("a", 1, DataType::I32).with_count(4),
            BufferDecl::read("b", 2, DataType::I32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::assign(
            "buf",
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V045")),
        "assigning a same-width int (rem result) to an i32 buffer must validate, got: {errors:?}"
    );
}

#[test]
fn store_bool_comparison_result_into_u32_buffer_validates() {
    // A comparison produces Bool; storing the 0/1 flag into a U32 output buffer
    // is a common, sound pattern (the emitter coerces Bool -> u32). The buffer
    // ASSIGN path always allowed it, but Node::Store rejected it until the compat
    // rule was unified into one `store_value_compatible` predicate.
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(4),
            BufferDecl::read("a", 1, DataType::I32).with_count(4),
            BufferDecl::read("b", 2, DataType::I32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::lt(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V045")
            || e.message().contains("value has type")),
        "storing a bool comparison result into a u32 buffer must validate, got: {errors:?}"
    );
}
