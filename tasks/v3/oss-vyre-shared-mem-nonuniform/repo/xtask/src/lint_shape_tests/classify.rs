// Assertion classification  -  SHAPE vs TRUTH. Plain `//` because this
// file is `include!()`-d into a `mod classify {}` scope; the
// `clippy::unnecessary_map_or` allow originally lived as an inner
// attribute here, but inner attributes cannot ride an `include!`-d
// chunk. The attribute is dropped in favour of writing each `map_or`
// site explicitly.

use syn::parse::Parser;
use syn::visit::Visit;
use syn::{Expr, ExprBinary, ExprUnary, ItemFn, Lit, Member, Stmt, UnOp};

use super::Classification;
#[cfg(test)]
use super::{is_test_function, visit_items};

/// Classify a test function by walking its body for assert*! macros.
pub(crate) fn classify_test(func: &ItemFn) -> (Classification, String) {
    let mut collector = AssertCollector::default();
    collector.visit_item_fn(func);

    if collector.asserts.is_empty() {
        return (
            Classification::NoAsserts,
            "no assert*! macros found".to_string(),
        );
    }

    let mut shape_reasons = Vec::new();
    for (name, args) in &collector.asserts {
        if is_shape_assert(name, args) {
            shape_reasons.push(format!("{name}!(... shape-only ...)"));
        } else {
            return (
                Classification::Truth,
                format!("truth assertion: {name}!(...)"),
            );
        }
    }

    (
        Classification::Shape,
        format!(
            "all assertions are shape-only: {}",
            shape_reasons.join(", ")
        ),
    )
}

/// Accumulate every `assert*!` macro invocation inside a syntax subtree.
#[derive(Default)]
struct AssertCollector {
    asserts: Vec<(String, Vec<Expr>)>,
}

impl<'ast> Visit<'ast> for AssertCollector {
    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if let Some(ident) = node.mac.path.get_ident() {
            let name = ident.to_string();
            if name.starts_with("assert") {
                let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
                if let Ok(args) = parser.parse2(node.mac.tokens.clone()) {
                    self.asserts.push((name, args.into_iter().collect()));
                }
            }
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if let Some(ident) = node.mac.path.get_ident() {
            let name = ident.to_string();
            if name.starts_with("assert") {
                let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
                if let Ok(args) = parser.parse2(node.mac.tokens.clone()) {
                    self.asserts.push((name, args.into_iter().collect()));
                }
            }
        }
        syn::visit::visit_stmt_macro(self, node);
    }
}

/// Return `true` if the given assert macro invocation matches a known
/// shape-only pattern.
fn is_shape_assert(name: &str, args: &[Expr]) -> bool {
    match name {
        "assert" => {
            let Some(first) = args.first() else {
                return false;
            };
            is_ok_call(first)
                || is_err_call(first)
                || is_not_empty(first)
                || is_len_gt_zero(first)
                || is_status_only_assert(first)
        }
        "assert_eq" => {
            let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
                return false;
            };
            is_roundtrip(left, right) || is_status_only_equality(left, right)
        }
        "assert_ne" => {
            let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
                return false;
            };
            is_status_only_equality(left, right)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Shape-pattern recognisers
// ---------------------------------------------------------------------------

fn is_ok_call(expr: &Expr) -> bool {
    matches!(expr, Expr::MethodCall(m) if m.method == "is_ok")
}

fn is_err_call(expr: &Expr) -> bool {
    matches!(expr, Expr::MethodCall(m) if m.method == "is_err")
}

fn is_not_empty(expr: &Expr) -> bool {
    let Expr::Unary(ExprUnary {
        op: UnOp::Not(_),
        expr: inner,
        ..
    }) = expr
    else {
        return false;
    };
    matches!(inner.as_ref(), Expr::MethodCall(m) if m.method == "is_empty")
}

fn is_len_gt_zero(expr: &Expr) -> bool {
    let Expr::Binary(ExprBinary {
        op, left, right, ..
    }) = expr
    else {
        return false;
    };
    let is_gt = matches!(op, syn::BinOp::Gt(_));
    let left_is_len = matches!(left.as_ref(), Expr::MethodCall(m) if m.method == "len");
    let right_is_zero = is_literal_zero(right);
    is_gt && left_is_len && right_is_zero
}

fn is_status_only_assert(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(method) => {
            method.method == "success" && expr_contains_status_word(&method.receiver)
        }
        Expr::Unary(ExprUnary {
            op: UnOp::Not(_),
            expr: inner,
            ..
        }) => is_status_only_assert(inner),
        Expr::Paren(paren) => is_status_only_assert(&paren.expr),
        Expr::Reference(reference) => is_status_only_assert(&reference.expr),
        Expr::Binary(binary) if matches!(binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) => {
            is_status_only_equality(&binary.left, &binary.right)
        }
        _ => false,
    }
}

fn is_status_only_equality(left: &Expr, right: &Expr) -> bool {
    (expr_is_status_like(left) && expr_is_status_literal(right))
        || (expr_is_status_like(right) && expr_is_status_literal(left))
}

fn expr_is_status_like(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(method) => {
            (method.method == "success" || method.method == "code")
                && expr_contains_status_word(&method.receiver)
        }
        Expr::Field(_) | Expr::Path(_) => expr_contains_status_word(expr),
        Expr::Paren(paren) => expr_is_status_like(&paren.expr),
        Expr::Reference(reference) => expr_is_status_like(&reference.expr),
        Expr::Unary(unary) => expr_is_status_like(&unary.expr),
        _ => false,
    }
}

fn expr_is_status_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Bool(_) => true,
            Lit::Int(int) => int.base10_parse::<i64>().is_ok_and(|value| value == 0),
            Lit::Str(string) => matches!(
                string.value().to_ascii_lowercase().as_str(),
                "ok" | "success" | "passed" | "pass" | "failed" | "failure"
            ),
            _ => false,
        },
        Expr::Call(call) => expr_contains_status_literal_argument(&call.args),
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| is_status_literal_word(&segment.ident.to_string())),
        Expr::Paren(paren) => expr_is_status_literal(&paren.expr),
        Expr::Reference(reference) => expr_is_status_literal(&reference.expr),
        _ => false,
    }
}

fn expr_contains_status_literal_argument(
    args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
) -> bool {
    args.iter().any(expr_is_status_literal)
}

fn expr_contains_status_word(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| is_status_word(&segment.ident.to_string())),
        Expr::Field(field) => {
            member_is_status_word(&field.member) || expr_contains_status_word(&field.base)
        }
        Expr::MethodCall(method) => {
            is_status_word(&method.method.to_string())
                || expr_contains_status_word(&method.receiver)
                || method.args.iter().any(expr_contains_status_word)
        }
        Expr::Call(call) => {
            expr_contains_status_word(&call.func) || call.args.iter().any(expr_contains_status_word)
        }
        Expr::Unary(unary) => expr_contains_status_word(&unary.expr),
        Expr::Binary(binary) => {
            expr_contains_status_word(&binary.left) || expr_contains_status_word(&binary.right)
        }
        Expr::Paren(paren) => expr_contains_status_word(&paren.expr),
        Expr::Reference(reference) => expr_contains_status_word(&reference.expr),
        Expr::Tuple(tuple) => tuple.elems.iter().any(expr_contains_status_word),
        Expr::Array(array) => array.elems.iter().any(expr_contains_status_word),
        Expr::Index(index) => {
            expr_contains_status_word(&index.expr) || expr_contains_status_word(&index.index)
        }
        _ => false,
    }
}

fn member_is_status_word(member: &Member) -> bool {
    match member {
        Member::Named(ident) => is_status_word(&ident.to_string()),
        Member::Unnamed(_) => false,
    }
}

fn is_status_word(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    word == "status"
        || word == "exit_status"
        || word == "exit_code"
        || word.ends_with("_status")
        || word.ends_with("_exit_code")
        || word.contains("status")
}

fn is_status_literal_word(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "ok" | "success" | "passed" | "pass" | "failed" | "failure"
    )
}

fn is_literal_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(l) if matches!(&l.lit, Lit::Int(i) if i.base10_parse::<i64>().unwrap_or(1) == 0))
}

/// Detect `assert_eq!(roundtrip(x), x)`  -  both arguments are the same
/// expression, or the right-hand side is an identifier that appears as an
/// argument in a call expression on the left-hand side.
fn is_roundtrip(left: &Expr, right: &Expr) -> bool {
    // Case 1: both sides are syntactically identical simple identifiers.
    if let (Expr::Path(a), Expr::Path(b)) = (left, right) {
        if a.path.get_ident() == b.path.get_ident() {
            return true;
        }
    }

    // Case 2: right is a simple identifier and left is a call that contains it.
    let Expr::Path(right_path) = right else {
        return false;
    };
    let Some(right_ident) = right_path.path.get_ident() else {
        return false;
    };
    let right_str = right_ident.to_string();

    if !is_call_expr(left) {
        return false;
    }
    expr_contains_ident(left, &right_str)
}

fn is_call_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(_) | Expr::MethodCall(_))
}

fn expr_contains_ident(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Path(p) => p.path.get_ident().map(|i| i == target).unwrap_or(false),
        Expr::Call(c) => {
            c.args.iter().any(|a| expr_contains_ident(a, target))
                || expr_contains_ident(&c.func, target)
        }
        Expr::MethodCall(m) => {
            m.args.iter().any(|a| expr_contains_ident(a, target))
                || expr_contains_ident(&m.receiver, target)
        }
        Expr::Unary(u) => expr_contains_ident(&u.expr, target),
        Expr::Binary(b) => {
            expr_contains_ident(&b.left, target) || expr_contains_ident(&b.right, target)
        }
        Expr::Paren(p) => expr_contains_ident(&p.expr, target),
        Expr::Reference(r) => expr_contains_ident(&r.expr, target),
        Expr::Block(b) => b.block.stmts.iter().any(|s| stmt_contains_ident(s, target)),
        Expr::If(i) => {
            expr_contains_ident(&i.cond, target)
                || i.then_branch
                    .stmts
                    .iter()
                    .any(|s| stmt_contains_ident(s, target))
                || i.else_branch
                    .as_ref()
                    .map_or(false, |(_, e)| expr_contains_ident(e, target))
        }
        Expr::Match(m) => {
            expr_contains_ident(&m.expr, target)
                || m.arms.iter().any(|a| {
                    a.guard
                        .as_ref()
                        .map_or(false, |(_, g)| expr_contains_ident(g, target))
                        || expr_contains_ident(&a.body, target)
                })
        }
        Expr::Tuple(t) => t.elems.iter().any(|e| expr_contains_ident(e, target)),
        Expr::Array(a) => a.elems.iter().any(|e| expr_contains_ident(e, target)),
        Expr::Index(i) => {
            expr_contains_ident(&i.expr, target) || expr_contains_ident(&i.index, target)
        }
        Expr::Field(f) => expr_contains_ident(&f.base, target),
        Expr::Cast(c) => expr_contains_ident(&c.expr, target),
        Expr::Let(l) => expr_contains_ident(&l.expr, target),
        Expr::Loop(l) => l.body.stmts.iter().any(|s| stmt_contains_ident(s, target)),
        Expr::While(w) => {
            expr_contains_ident(&w.cond, target)
                || w.body.stmts.iter().any(|s| stmt_contains_ident(s, target))
        }
        Expr::ForLoop(f) => {
            expr_contains_ident(&f.expr, target)
                || f.body.stmts.iter().any(|s| stmt_contains_ident(s, target))
        }
        Expr::Closure(c) => expr_contains_ident(&c.body, target),
        Expr::Async(a) => a.block.stmts.iter().any(|s| stmt_contains_ident(s, target)),
        Expr::Await(a) => expr_contains_ident(&a.base, target),
        Expr::Try(t) => expr_contains_ident(&t.expr, target),
        Expr::TryBlock(t) => t.block.stmts.iter().any(|s| stmt_contains_ident(s, target)),
        Expr::Struct(s) => s
            .fields
            .iter()
            .any(|f| expr_contains_ident(&f.expr, target)),
        _ => false,
    }
}

fn stmt_contains_ident(stmt: &Stmt, target: &str) -> bool {
    match stmt {
        Stmt::Local(l) => l
            .init
            .as_ref()
            .map_or(false, |i| expr_contains_ident(&i.expr, target)),
        Stmt::Expr(e, _) => expr_contains_ident(e, target),
        Stmt::Item(_) => false,
        Stmt::Macro(m) => {
            // Best-effort: we can't parse the macro body generically, but if it
            // contains the target identifier as a token it'll still count.
            m.mac.tokens.to_string().contains(target)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn parse_func(source: &str) -> ItemFn {
        syn::parse_str(source)
            .expect("Fix: valid test function; restore this invariant before continuing.")
    }

    #[test]
    fn shape_is_ok() {
        let f = parse_func(r#"#[test] fn t() { assert!(result.is_ok()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_is_err() {
        let f = parse_func(r#"#[test] fn t() { assert!(result.is_err()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_not_empty() {
        let f = parse_func(r#"#[test] fn t() { assert!(!findings.is_empty()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_len_gt_zero() {
        let f = parse_func(r#"#[test] fn t() { assert!(vec.len() > 0); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_parse_is_ok() {
        let f = parse_func(r#"#[test] fn t() { assert!(parse(s).is_ok()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_roundtrip() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(parse_then_serialize(x), x); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_status_success() {
        let f = parse_func(r#"#[test] fn t() { assert!(output.status.success()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_negative_status_success() {
        let f = parse_func(r#"#[test] fn t() { assert!(!output.status.success()); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_status_code_equality() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(output.status.code(), Some(0)); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_exit_code_equality() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(result.exit_code, 0); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn shape_named_status_success_equality() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(result.status, "success"); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn truth_distinct_non_status_values_are_not_shape() {
        let f = parse_func(r#"#[test] fn t() { assert_ne!(FloatType::F32, FloatType::F64); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Truth);
    }

    #[test]
    fn truth_specific_value() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(x.line_start, 12); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Truth);
    }

    #[test]
    fn truth_cwe_field() {
        let f = parse_func(r#"#[test] fn t() { assert_eq!(f.cwe, Some("CWE-190")); }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Truth);
    }

    #[test]
    fn truth_filter_count() {
        let f = parse_func(
            r#"#[test] fn t() { assert_eq!(findings.filter(|f| f.rule_name == "X").count(), 1); }"#,
        );
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Truth);
    }

    #[test]
    fn mixed_shape_and_truth_is_truth() {
        let f = parse_func(
            r#"
            #[test] fn t() {
                assert!(result.is_ok());
                assert!(output.status.success());
                assert_eq!(x.line_start, 12);
            }
        "#,
        );
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Truth);
    }

    #[test]
    fn no_asserts_classified_noasserts() {
        let f = parse_func(r#"#[test] fn t() { let _ = 1 + 1; }"#);
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::NoAsserts);
    }

    #[test]
    fn tokio_test_detected() {
        let f = parse_func(r#"#[tokio::test] async fn t() { assert!(result.is_ok()); }"#);
        assert!(is_test_function(&f));
        let (cls, _) = classify_test(&f);
        assert_eq!(cls, Classification::Shape);
    }

    #[test]
    fn nested_mod_test_found() {
        let source = r#"
            mod inner {
                #[test]
                fn t() {
                    assert!(result.is_ok());
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let mut findings = Vec::new();
        visit_items(&file.items, "", Path::new("test.rs"), "demo", &mut findings);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].module_path, "inner");
        assert_eq!(findings[0].classification, Classification::Shape);
    }
}
