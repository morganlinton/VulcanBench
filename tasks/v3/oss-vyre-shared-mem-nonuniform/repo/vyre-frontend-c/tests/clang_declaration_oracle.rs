//! Tests for clang declaration fact extraction.

mod support;

use std::fs;

use support::ast_oracle::clang_user_declarations_required;

#[test]
fn clang_declaration_oracle_records_user_declaration_nodes() {
    let path = std::env::temp_dir().join(format!(
        "vyrec-clang-declaration-oracle-{}.c",
        std::process::id()
    ));
    fs::write(
        &path,
        "typedef int myint;\nstruct S { myint field; };\nstatic myint f(myint a) { return a; }\n",
    )
    .expect("test source must be writable");

    let declarations = clang_user_declarations_required(&path);
    fs::remove_file(&path).expect("test source must be removable");

    assert!(
        declarations.iter().any(|decl| decl.kind == "TypedefDecl"
            && decl.name.as_deref() == Some("myint")
            && decl.qual_type.as_deref() == Some("int")),
        "typedef declaration fact must include name and qualified type: {declarations:#?}"
    );
    assert!(
        declarations
            .iter()
            .any(|decl| decl.kind == "RecordDecl" && decl.name.as_deref() == Some("S")),
        "struct declaration fact must be present: {declarations:#?}"
    );
    assert!(
        declarations.iter().any(|decl| decl.kind == "FieldDecl"
            && decl.name.as_deref() == Some("field")
            && decl.qual_type.as_deref() == Some("myint")),
        "field declaration fact must include typedef-qualified type: {declarations:#?}"
    );
    let function = declarations
        .iter()
        .find(|decl| decl.kind == "FunctionDecl" && decl.name.as_deref() == Some("f"))
        .expect("function declaration fact must be present");
    assert_eq!(function.line, Some(3));
    assert_eq!(function.column, Some(14));
}
