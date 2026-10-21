//! Debug test for c11_annotate_typedef_names emission.
#![cfg(feature = "c-parser")]
#![allow(missing_docs)]

use vyre::ir::Expr;
use vyre_libs::parsing::c::parse::vast::c11_annotate_typedef_names;

#[test]
fn debug_c11_annotate_typedef_names_emit_literal() {
    let program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::u32(1024),
        Expr::u32(100),
        "annotated_vast",
    );

    let lowered = vyre_lower::pre_emit::lower_for_emit(&program).unwrap();
    let module = vyre_emit_naga::emit(&lowered.descriptor).unwrap();

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    match validator.validate(&module) {
        Ok(_) => println!("Literal params: Validation passed!"),
        Err(e) => {
            eprintln!("Literal params: Validation failed: {:?}", e);
            panic!("validation failed");
        }
    }
}

#[test]
fn debug_c11_annotate_typedef_names_emit_dynamic() {
    let program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::BufLen {
            buffer: "haystack".into(),
        },
        Expr::BufLen {
            buffer: "vast_nodes".into(),
        },
        "annotated_vast",
    );

    let lowered = vyre_lower::pre_emit::lower_for_emit(&program).unwrap();
    let module = vyre_emit_naga::emit(&lowered.descriptor).unwrap();

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    match validator.validate(&module) {
        Ok(_) => println!("Dynamic params: Validation passed!"),
        Err(e) => {
            eprintln!("Dynamic params: Validation failed: {:?}", e);
            panic!("validation failed");
        }
    }
}
