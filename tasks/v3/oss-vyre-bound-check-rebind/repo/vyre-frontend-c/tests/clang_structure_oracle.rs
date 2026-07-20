//! Tests for clang statement/expression structure fact extraction.

mod support;

use std::fs;

use support::ast_oracle::clang_user_structure_required;

#[test]
fn clang_structure_oracle_records_statement_and_expression_nodes() {
    let path = std::env::temp_dir().join(format!(
        "vyrec-clang-structure-oracle-{}.c",
        std::process::id()
    ));
    fs::write(&path, "int g(int a) { int b = a + 1; return b ? b : 0; }\n")
        .expect("test source must be writable");

    let structure = clang_user_structure_required(&path);
    fs::remove_file(&path).expect("test source must be removable");

    for expected_kind in [
        "CompoundStmt",
        "DeclStmt",
        "BinaryOperator",
        "DeclRefExpr",
        "IntegerLiteral",
        "ReturnStmt",
        "ConditionalOperator",
    ] {
        assert!(
            structure.iter().any(|fact| fact.kind == expected_kind),
            "expected clang structure kind {expected_kind} in {structure:#?}"
        );
    }

    assert!(
        structure.iter().any(|fact| fact.kind == "DeclRefExpr"
            && fact.referenced_decl_name.as_deref() == Some("a")
            && fact.qual_type.as_deref() == Some("int")),
        "DeclRefExpr must carry referenced declaration and expression type: {structure:#?}"
    );

    let return_stmt = structure
        .iter()
        .find(|fact| fact.kind == "ReturnStmt")
        .expect("ReturnStmt must be present");
    assert_eq!(return_stmt.line, Some(1));
}
