//! Tests for clang type fact extraction.

mod support;

use std::fs;

use support::ast_oracle::clang_user_type_facts_required;

#[test]
fn clang_type_oracle_records_core_c_type_shapes() {
    let path =
        std::env::temp_dir().join(format!("vyrec-clang-type-oracle-{}.c", std::process::id()));
    fs::write(
        &path,
        concat!(
            "typedef const int *cip;\n",
            "enum E { EA = 1 };\n",
            "struct S { int field; };\n",
            "union U { int i; long l; };\n",
            "cip ptr;\n",
            "int arr[3];\n",
            "int fn(int a);\n",
            "__typeof__(1) tx;\n",
        ),
    )
    .expect("test source must be writable");

    let facts = clang_user_type_facts_required(&path);
    fs::remove_file(&path).expect("test source must be removable");

    assert!(
        facts.iter().any(|fact| fact.owner_kind == "TypedefDecl"
            && fact.owner_name.as_deref() == Some("cip")
            && fact.is_const
            && fact.pointer_depth == 1),
        "typedef pointer-to-const fact must be extracted: {facts:#?}"
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.owner_name.as_deref() == Some("ptr")
                && fact.uses_typedef
                && fact.desugared_qual_type.as_deref() == Some("const int *")),
        "typedef use must retain alias and desugared type facts: {facts:#?}"
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.owner_name.as_deref() == Some("arr")
                && fact.array_depth == 1
                && fact.qual_type == "int[3]"),
        "array fact must include array depth and extent spelling: {facts:#?}"
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.owner_name.as_deref() == Some("fn")
                && fact.is_function
                && fact.qual_type == "int (int)"),
        "function type fact must be extracted: {facts:#?}"
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.tag_kind.as_deref() == Some("enum")),
        "enum tag type fact must be extracted: {facts:#?}"
    );
    assert!(
        facts.iter().any(|fact| fact.uses_typeof),
        "typeof type fact must be extracted: {facts:#?}"
    );
}
