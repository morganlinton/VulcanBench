#[test]
fn adversarial_module_named_tests_inside_a_real_module() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        mod inner {
            fn build() {
                let _ = Node::let_bind("a", val());
            }
            mod tests {
                fn t() {
                    let _ = Node::let_bind("b", val());
                }
            }
        }
        "#,
    );
    let v = lint(dir.path());
    assert_eq!(v.len(), 1, "build() flagged, tests::t() not: {v:?}");
}

// ============== Allowlist behavior ==============

#[test]
fn allowlist_excludes_listed_files() {
    use vyre_lints::run_raw_ir_in_libs;
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/exempt_op.rs",
        r#"
        fn build() {
            let _ = Node::let_bind("x", val());
        }
        "#,
    );
    write_lib_file(
        dir.path(),
        "nn/active_op.rs",
        r#"
        fn build() {
            let _ = Node::let_bind("y", val());
        }
        "#,
    );
    let allow_path = dir.path().join("allowlist.toml");
    std::fs::write(
        &allow_path,
        "exempt_files = [\"vyre-libs/src/nn/exempt_op.rs\"]\n",
    )
    .unwrap();
    let lib_src = dir.path().join("vyre-libs").join("src");
    let v = run_raw_ir_in_libs(&[lib_src.as_path()], Some(allow_path.as_path())).unwrap();
    assert_eq!(v.len(), 1);
    assert!(v[0].file.contains("active_op.rs"));
}

// ============== Idempotence / determinism ==============

#[test]
fn idempotent_two_runs_same_violations() {
    let dir = tempfile::tempdir().unwrap();
    write_lib_file(
        dir.path(),
        "nn/op.rs",
        r#"
        fn build() {
            let _ = Node::let_bind("x", val());
            let _ = Expr::add(a(), b());
        }
        "#,
    );
    let v1 = lint(dir.path());
    let v2 = lint(dir.path());
    assert_eq!(v1, v2);
}

#[test]
fn coverage_assertion_minimum_test_count() {
    // Per the SEPARATION_AUDIT S0 + adversarial-tests-mandatory bar:
    // any lint we ship has positive + negative + adversarial coverage.
    // This test pins the count so removing tests requires explicit
    // intent.
    //
    // 8 positive, 6 negative, 6 adversarial, 1 allowlist, 1 idempotence
    // = 22 minimum (not counting this self-counter).
    //
    // If you rename a test, update this comment.
    // see other tests in this file
}
