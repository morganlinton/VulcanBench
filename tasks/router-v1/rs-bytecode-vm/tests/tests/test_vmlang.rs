//! Hidden tests: a bytecode compiler and a stack VM whose only coupling is the
//! shared instruction set in `vmlang::isa`. Short-circuit `and`/`or` and `if`
//! need forward jumps with backpatched absolute targets; an offset that is off
//! by one only shows up when the VM runs the code.

use vmlang::{compile_expr, run, BinOp, Env, Expr, Instr, Value, VmError};

fn eval(expr: &Expr) -> Result<Value, VmError> {
    run(&compile_expr(expr), &Env::new())
}

fn eval_in(expr: &Expr, bindings: &[(&str, Value)]) -> Result<Value, VmError> {
    let env: Env = bindings
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    run(&compile_expr(expr), &env)
}

#[test]
fn construct() {
    // passes pre-fix: the shared contract is already in place
    let e = Expr::bin(BinOp::Add, Expr::lit(1), Expr::var("x"));
    assert!(matches!(e, Expr::Bin(BinOp::Add, _, _)));
    assert_eq!(Instr::Push(Value::Num(1.0)), Instr::Push(1.into()));
    assert!(Value::Str("a".into()).truthy() && !Value::Num(0.0).truthy());
}

#[test]
fn arithmetic() {
    // 2*3 + 4
    let e = Expr::bin(
        BinOp::Add,
        Expr::bin(BinOp::Mul, Expr::lit(2), Expr::lit(3)),
        Expr::lit(4),
    );
    assert_eq!(eval(&e), Ok(Value::Num(10.0)));
    let d = Expr::bin(BinOp::Div, Expr::lit(7), Expr::lit(2));
    assert_eq!(eval(&d), Ok(Value::Num(3.5)));
}

#[test]
fn variables_and_compare() {
    let e = Expr::bin(BinOp::Gt, Expr::var("x"), Expr::lit(5));
    assert_eq!(eval_in(&e, &[("x", 9.into())]), Ok(Value::Bool(true)));
    assert_eq!(eval_in(&e, &[("x", 1.into())]), Ok(Value::Bool(false)));
    let s = Expr::bin(BinOp::Eq, Expr::var("s"), Expr::lit("hi"));
    assert_eq!(eval_in(&s, &[("s", "hi".into())]), Ok(Value::Bool(true)));
}

#[test]
fn unknown_variable_errors() {
    assert_eq!(
        eval(&Expr::var("missing")),
        Err(VmError::UnboundVariable("missing".into()))
    );
}

#[test]
fn division_by_zero_errors() {
    let e = Expr::bin(BinOp::Div, Expr::lit(1), Expr::lit(0));
    assert_eq!(eval(&e), Err(VmError::DivisionByZero));
}

#[test]
fn not() {
    assert_eq!(eval(&Expr::not(Expr::lit(0))), Ok(Value::Bool(true)));
    assert_eq!(eval(&Expr::not(Expr::lit(7))), Ok(Value::Bool(false)));
    assert_eq!(eval(&Expr::not(Expr::lit(""))), Ok(Value::Bool(true)));
}

#[test]
fn if_selects_branch_by_condition() {
    let e = Expr::if_(Expr::var("c"), Expr::lit("yes"), Expr::lit("no"));
    assert_eq!(eval_in(&e, &[("c", true.into())]), Ok("yes".into()));
    assert_eq!(eval_in(&e, &[("c", false.into())]), Ok("no".into()));
}

#[test]
fn nested_if_jump_targets() {
    // nested ifs stress that backpatched jump targets stay correct as code grows
    let inner = Expr::if_(Expr::var("b"), Expr::lit(2), Expr::lit(3));
    let e = Expr::if_(Expr::var("a"), inner, Expr::lit(9));
    assert_eq!(eval_in(&e, &[("a", true.into()), ("b", true.into())]), Ok(2.into()));
    assert_eq!(eval_in(&e, &[("a", true.into()), ("b", false.into())]), Ok(3.into()));
    assert_eq!(eval_in(&e, &[("a", false.into()), ("b", true.into())]), Ok(9.into()));
}

#[test]
fn and_or_values() {
    // and/or return an operand, not a forced bool
    assert_eq!(eval(&Expr::and(Expr::lit(0), Expr::lit("x"))), Ok(0.into()));
    assert_eq!(eval(&Expr::and(Expr::lit(1), Expr::lit("x"))), Ok("x".into()));
    assert_eq!(eval(&Expr::or(Expr::lit(0), Expr::lit("x"))), Ok("x".into()));
    assert_eq!(eval(&Expr::or(Expr::lit(5), Expr::lit("x"))), Ok(5.into()));
}

#[test]
fn short_circuit_skips_faulty_branch() {
    // the unselected operand must not run: it divides by zero / reads an unbound name
    let boom = Expr::bin(BinOp::Div, Expr::lit(1), Expr::lit(0));
    assert_eq!(eval(&Expr::and(Expr::lit(0), boom.clone())), Ok(0.into()));
    assert_eq!(eval(&Expr::or(Expr::lit(1), Expr::var("nope"))), Ok(1.into()));
    // and the selected one does
    assert_eq!(eval(&Expr::and(Expr::lit(1), boom)), Err(VmError::DivisionByZero));
}

#[test]
fn nested_short_circuit_in_condition() {
    // (a and b) or c, with a jump target inside another backpatched form
    let e = Expr::or(
        Expr::and(Expr::var("a"), Expr::var("b")),
        Expr::var("c"),
    );
    let t = Value::Bool(true);
    let f = Value::Bool(false);
    assert_eq!(eval_in(&e, &[("a", t.clone()), ("b", t.clone()), ("c", f.clone())]), Ok(t.clone()));
    assert_eq!(eval_in(&e, &[("a", t.clone()), ("b", f.clone()), ("c", "z".into())]), Ok("z".into()));
    assert_eq!(eval_in(&e, &[("a", f.clone()), ("b", t.clone()), ("c", f.clone())]), Ok(f));
}

#[test]
fn flat_isa_with_forward_absolute_jumps() {
    let e = Expr::if_(
        Expr::and(Expr::var("a"), Expr::var("b")),
        Expr::or(Expr::lit(0), Expr::lit(1)),
        Expr::lit(2),
    );
    let code = compile_expr(&e);
    assert!(code.len() > 8);
    for (i, instr) in code.iter().enumerate() {
        if let Instr::Jump(t) | Instr::JumpIfFalse(t) = instr {
            assert!(*t > i, "jump at {i} must go forward, got {t}");
            assert!(*t <= code.len(), "jump at {i} targets {t} past the end");
        }
    }
    // the program leaves exactly one value
    let env: Env = [("a".to_string(), Value::Bool(true)), ("b".to_string(), Value::Bool(true))]
        .into_iter()
        .collect();
    assert_eq!(run(&code, &env), Ok(1.into()));
}
