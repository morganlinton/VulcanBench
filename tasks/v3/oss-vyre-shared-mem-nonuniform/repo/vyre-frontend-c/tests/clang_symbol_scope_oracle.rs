//! Tests for clang symbol, scope, linkage, redeclaration, and ownership extraction.

mod support;

use std::fs;

use support::ast_oracle::clang_user_symbol_scope_facts_required;

#[test]
fn clang_symbol_scope_oracle_records_storage_linkage_redecl_and_ownership() {
    let path = std::env::temp_dir().join(format!(
        "vyrec-clang-symbol-scope-oracle-{}.c",
        std::process::id()
    ));
    fs::write(
        &path,
        concat!(
            "extern int x;\n",
            "extern int x;\n",
            "static int y;\n",
            "struct S { int field; };\n",
            "static void f(int p) { int z; }\n",
        ),
    )
    .expect("test source must be writable");

    let facts = clang_user_symbol_scope_facts_required(&path);
    fs::remove_file(&path).expect("test source must be removable");

    let redeclared_x = facts
        .iter()
        .find(|fact| {
            fact.kind == "VarDecl"
                && fact.name == "x"
                && fact.previous_decl.is_some()
                && fact.line == Some(2)
        })
        .expect("second extern declaration must carry previousDecl");
    assert_eq!(redeclared_x.storage_class.as_deref(), Some("extern"));
    assert_eq!(redeclared_x.scope_kind, "file");
    assert_eq!(redeclared_x.linkage, "external");
    assert_eq!(redeclared_x.visibility, "external");

    let static_y = facts
        .iter()
        .find(|fact| fact.kind == "VarDecl" && fact.name == "y")
        .expect("static file-scope variable must be present");
    assert_eq!(static_y.storage_class.as_deref(), Some("static"));
    assert_eq!(static_y.linkage, "internal");
    assert_eq!(static_y.visibility, "translation-unit");

    let field = facts
        .iter()
        .find(|fact| fact.kind == "FieldDecl" && fact.name == "field")
        .expect("field declaration must be owned by the record declaration");
    assert_eq!(field.owner_kind.as_deref(), Some("RecordDecl"));
    assert_eq!(field.owner_name.as_deref(), Some("S"));
    assert_eq!(field.scope_kind, "aggregate");

    let local_z = facts
        .iter()
        .find(|fact| fact.kind == "VarDecl" && fact.name == "z")
        .expect("block-local variable must be owned by the function declaration");
    assert_eq!(local_z.owner_kind.as_deref(), Some("FunctionDecl"));
    assert_eq!(local_z.owner_name.as_deref(), Some("f"));
    assert_eq!(local_z.scope_kind, "function");
    assert_eq!(local_z.linkage, "none");
}
