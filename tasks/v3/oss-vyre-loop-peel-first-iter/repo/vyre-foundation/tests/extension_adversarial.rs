//! Adversarial extension-node shape tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

#[test]
fn call_extension_arguments_survive_wire_roundtrip() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![
            Node::call("plugin.saturating_u32", vec![Expr::u32(7), Expr::u32(11)]),
            Node::store("out", Expr::u32(0), Expr::u32(18)),
        ],
    );

    let encoded = program.to_wire().expect("extension call must encode");
    let decoded = Program::from_wire(&encoded).expect("extension call must decode");

    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "extension call argument shape must be stable across wire round-trip"
    );
}

#[test]
fn async_extension_tags_remain_structural() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        vec![
            Node::async_load_ext("ssd", "vram", Expr::u32(0), Expr::u32(4096), "load.tag"),
            Node::async_wait("load.tag"),
            Node::async_store("vram", "ssd", Expr::u32(0), Expr::u32(4096), "store.tag"),
            Node::async_wait("store.tag"),
            Node::store("out", Expr::u32(0), Expr::u32(1)),
        ],
    );

    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "async extension tags must validate as structural IR, got {errors:?}"
    );
}
