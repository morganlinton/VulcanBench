//! Tests for clang ABI/layout oracle extraction.

mod support;

use std::fs;

use support::clang_abi::{clang_enum_and_function_abi, clang_record_layouts};

#[test]
fn clang_abi_oracle_records_layout_enum_and_function_facts() {
    let path =
        std::env::temp_dir().join(format!("vyrec-clang-abi-oracle-{}.c", std::process::id()));
    fs::write(
        &path,
        concat!(
            "struct S { char c; int x; unsigned b:3; };\n",
            "union U { char c; long l; };\n",
            "enum E { A = -1, B = 5 };\n",
            "struct S gs; union U gu; enum E ge;\n",
            "static int f(int a) { return a; }\n",
        ),
    )
    .expect("test source must be writable");

    let layouts = clang_record_layouts(&path).expect("record layout oracle must run");
    let (enums, functions) = clang_enum_and_function_abi(&path).expect("ABI AST oracle must run");
    fs::remove_file(&path).expect("test source must be removable");

    let struct_s = layouts
        .iter()
        .find(|layout| layout.kind == "struct" && layout.name == "S")
        .expect("struct S layout must be present");
    assert_eq!(struct_s.size_bytes, 12);
    assert_eq!(struct_s.align_bytes, 4);
    assert!(
        struct_s
            .fields
            .iter()
            .any(|field| field.name == "x" && field.byte_offset == 4),
        "struct field offset must be recorded: {struct_s:#?}"
    );
    assert!(
        struct_s.fields.iter().any(|field| {
            field.name == "b"
                && field.byte_offset == 8
                && field.bit_start == Some(0)
                && field.bit_end == Some(2)
        }),
        "bitfield bit range must be recorded: {struct_s:#?}"
    );

    let union_u = layouts
        .iter()
        .find(|layout| layout.kind == "union" && layout.name == "U")
        .expect("union U layout must be present");
    assert_eq!(union_u.size_bytes, 8);
    assert_eq!(union_u.align_bytes, 8);
    assert!(
        union_u.fields.iter().all(|field| field.byte_offset == 0),
        "union fields must share byte offset zero: {union_u:#?}"
    );

    let enum_e = enums
        .iter()
        .find(|enum_fact| enum_fact.name == "E")
        .expect("enum E ABI fact must be present");
    assert_eq!(enum_e.representation, "int");
    assert!(enum_e
        .enumerators
        .contains(&("A".to_string(), "-1".to_string())));
    assert!(enum_e
        .enumerators
        .contains(&("B".to_string(), "5".to_string())));

    let function_f = functions
        .iter()
        .find(|function| function.name == "f")
        .expect("function ABI fact must be present");
    assert_eq!(function_f.qual_type, "int (int)");
    assert_eq!(function_f.storage_class.as_deref(), Some("static"));
}
