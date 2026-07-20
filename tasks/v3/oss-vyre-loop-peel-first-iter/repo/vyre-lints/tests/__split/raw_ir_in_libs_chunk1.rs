// End-to-end tests for the `raw_ir_in_libs` lint.
//
// Each test writes a synthetic vyre-libs source file to a tempdir,
// runs the lint, and asserts on the exact violation set.
//
// `//` line comments rather than `//!` inner doc  -  this file is
// `include!()`-d via a parent `raw_ir_in_libs.rs` shim, and inner
// docs would attach to that parent module.

use std::path::PathBuf;
use vyre_lints::{run_raw_ir_in_libs, Violation, ViolationKind};

fn write_lib_file(dir: &std::path::Path, rel: &str, source: &str) -> PathBuf {
    let path = dir.join("vyre-libs").join("src").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, source).unwrap();
    path
}

fn lint(dir: &std::path::Path) -> Vec<Violation> {
    let lib_src = dir.join("vyre-libs").join("src");
    run_raw_ir_in_libs(&[lib_src.as_path()], None).expect("lint runs")
}

// ============== Positive truth (rule fires) ==============

#[test]
fn positive_node_struct_literal_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = Node::Store {
                buffer: "out".into(),
                index: 0,
                value: 1,
            };
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1, "got {v:?}");
    assert_eq!(v[0].kind, ViolationKind::RawNodeConstruction);
    assert!(
        v[0].file.ends_with("vyre-libs/src/nn/op.rs"),
        "{}",
        v[0].file
    );
}

#[test]
fn positive_node_call_constructor_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = Node::let_bind("x", expr_for_x());
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::RawNodeConstruction);
}

#[test]
fn positive_expr_call_constructor_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "visual/op.rs",
        r#"
        fn build() {
            let _ = Expr::add(Expr::var("a"), Expr::u32(4));
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 3, "got {v:?}");
    for vv in &v {
        assert_eq!(vv.kind, ViolationKind::RawExprConstruction);
    }
}

#[test]
fn positive_qualified_path_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = vyre::ir::Node::let_bind("x", value());
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1, "got {v:?}");
    assert_eq!(v[0].kind, ViolationKind::RawNodeConstruction);
}

#[test]
fn positive_multiple_violations_in_one_file_all_reported() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let a = Node::let_bind("a", Expr::u32(0));
            let b = Node::let_bind("b", Expr::u32(1));
            let _ = Expr::add(Expr::var("a"), Expr::var("b"));
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 7, "got {v:?}");
    let nodes = v
        .iter()
        .filter(|x| x.kind == ViolationKind::RawNodeConstruction)
        .count();
    let exprs = v
        .iter()
        .filter(|x| x.kind == ViolationKind::RawExprConstruction)
        .count();
    assert_eq!(nodes, 2);
    assert_eq!(exprs, 5);
}

#[test]
fn positive_violations_sorted_by_file_then_line() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "z_later/op.rs",
        "fn z() { let _ = Node::let_bind(\"x\", v()); }",
    );
    write_lib_file(
        dir.path(),
        "a_first/op.rs",
        "fn a() { let _ = Node::let_bind(\"y\", v()); let _ = Node::let_bind(\"z\", v()); }",
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 3);
    assert!(v[0].file.contains("a_first"));
    assert!(v[1].file.contains("a_first"));
    assert!(v[2].file.contains("z_later"));
}

#[test]
fn positive_inside_nested_function_still_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn outer() {
            fn inner() -> u32 {
                let _ = Node::let_bind("x", val());
                42
            }
            inner();
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1);
}

#[test]
fn positive_inside_closure_still_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let f = || Node::let_bind("x", val());
            f();
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1);
}

// ============== Negative precision (rule does NOT fire) ==============

#[test]
fn negative_pattern_match_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn classify(n: &Node) -> &'static str {
            match n {
                Node::Store { .. } => "store",
                Node::Region { .. } => "region",
                _ => "other",
            }
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(v.is_empty(), "patterns must not be flagged: {v:?}");
}

#[test]
fn negative_test_module_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        #[cfg(test)]
        mod tests {
            fn build() {
                let _ = Node::let_bind("x", value());
                let _ = Expr::add(a(), b());
            }
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(v.is_empty(), "test module must not be flagged: {v:?}");
}

#[test]
fn negative_test_fn_attribute_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        #[test]
        fn it_works() {
            let _ = Node::let_bind("x", value());
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(v.is_empty());
}

#[test]
fn negative_use_import_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        use vyre::ir::{Node, Expr};
        fn nothing() {}
        "#,
    );
    let v = lint(dir.path());
    assert!(v.is_empty(), "imports must not be flagged: {v:?}");
}

#[test]
fn negative_other_type_with_node_in_name_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = MyNode::Store { val: 1 };
            let _ = ExprList::new();
            let _ = Expression::new();
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(v.is_empty(), "non-IR types must not be flagged: {v:?}");
}

#[test]
fn negative_function_returning_node_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build_one() -> Option<Node> {
            None
        }
        fn caller() {
            let _: Vec<Node> = Vec::new();
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(
        v.is_empty(),
        "type position references must not be flagged: {v:?}"
    );
}

#[test]
fn negative_primitive_builder_call_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = vyre_primitives::storage::store_strided("buf", 0, 1);
            let _ = vyre_primitives::nn::attention_max_pass();
        }
        "#,
    );
    let v = lint(dir.path());
    assert!(
        v.is_empty(),
        "primitive builder calls must not be flagged: {v:?}"
    );
}

// ============== Adversarial (evade attempts) ==============

#[test]
fn adversarial_node_inside_match_arm_construction_flagged() {
    // Pattern matching is fine, but if the match ARM constructs a Node::*,
    // that IS a violation.
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn rebuild(n: Node) -> Node {
            match n {
                Node::Store { .. } => Node::let_bind("x", val()),
                _ => Node::Region { body: vec![], parent: None },
            }
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(
        v.len(),
        2,
        "match arms that CONSTRUCT must be flagged: {v:?}"
    );
}

#[test]
fn adversarial_inside_macro_body_flagged_when_visible() {
    // Inside an inline macro_rules!, the syn parser sees the body as
    // tokens. We don't expand macros  -  this test pins behavior so a
    // future contributor knows: macro bodies are not scanned. If they
    // hide construction in a macro to evade the lint, we accept that
    // limitation today (escalation: dispatch a second lint that checks
    // macro outputs at expansion time).
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        macro_rules! evade {
            () => { Node::let_bind("x", val()) };
        }
        fn caller() {
            let _ = evade!();
        }
        "#,
    );
    let v = lint(dir.path());
    // Documented limitation: the lint does not expand macros. If this
    // becomes a real evasion vector in the field, the fix is a second
    // pass that runs after `cargo expand` on each crate. Keeping the
    // assertion here so the limitation is committed truth.
    assert!(v.is_empty(), "documented: macro bodies are not scanned");
}

#[test]
fn adversarial_inside_const_initializer_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        const _: bool = {
            let _ = Expr::u32(7);
            true
        };
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1);
}

#[test]
fn adversarial_inside_static_initializer_flagged() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        static _DUMMY: u8 = {
            let _ = Node::let_bind("x", val());
            0
        };
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1);
}

#[test]
fn adversarial_test_attribute_does_not_leak_to_sibling_fn() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        #[test]
        fn ok_in_test() {
            let _ = Node::let_bind("x", val());
        }

        fn not_a_test() {
            let _ = Node::let_bind("y", val());
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1, "only the non-test fn must be flagged: {v:?}");
}

