//! Exhaustive wire-format round-trip tests for various program shapes.
//!
//! Every valid program must survive `to_wire` → `from_wire` without
//! loss. These tests exercise shapes that are easy to miss: loops,
//! conditionals, barriers, traps, async ops, and indirect dispatch.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn roundtrip(prog: &Program) {
    let bytes = prog.to_wire().expect("must encode");
    let decoded = Program::from_wire(&bytes).expect("must decode");
    assert!(
        prog.structural_eq(&decoded),
        "round-trip failed for program with {} buffers and {} top-level nodes",
        prog.buffers().len(),
        prog.entry().len()
    );
}

#[test]
fn roundtrip_empty_program() {
    roundtrip(&Program::empty());
}

#[test]
fn roundtrip_return_only() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    ));
}

#[test]
fn roundtrip_single_store() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(42)),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_if_then() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::if_then(
                Expr::bool(true),
                vec![Node::store("out", Expr::u32(0), Expr::u32(1)), Node::Return],
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_loop() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(10),
                vec![Node::store("out", Expr::var("i"), Expr::var("i"))],
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_barrier() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::barrier(), Node::Return],
    ));
}

#[test]
fn roundtrip_trap() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(0), "test trap"), Node::Return],
    ));
}

#[test]
fn roundtrip_async_load_and_wait() {
    roundtrip(&Program::wrapped(
        vec![
            BufferDecl::read("src", 0, DataType::U32).with_count(1),
            BufferDecl::output("dst", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::async_load("stage-1"),
            Node::async_wait("stage-1"),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_indirect_dispatch() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::read("count", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::indirect_dispatch("count", 0), Node::Return],
    ));
}

#[test]
fn roundtrip_multiple_buffers() {
    roundtrip(&Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(64),
            BufferDecl::read("b", 1, DataType::U32).with_count(64),
            BufferDecl::output("c", 2, DataType::U32).with_count(64),
            BufferDecl::workgroup("scratch", 16, DataType::U32),
        ],
        [64, 1, 1],
        vec![
            Node::store(
                "c",
                Expr::u32(0),
                Expr::add(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_let_bind_and_assign() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::u32(5)),
            Node::assign("x", Expr::u32(10)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_nested_if_in_loop() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::if_then(
                    Expr::eq(Expr::var("i"), Expr::u32(2)),
                    vec![Node::store("out", Expr::var("i"), Expr::u32(99))],
                )],
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_select_expression() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::select(Expr::bool(true), Expr::u32(1), Expr::u32(0)),
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_cast_expression() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::cast(vyre::ir::DataType::F32, Expr::u32(42)),
            ),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_atomic_add_expression() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("_", Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1))),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_subgroup_add() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::subgroup_add(Expr::u32(1))),
            Node::Return,
        ],
    ));
}

#[test]
fn roundtrip_call_expression() {
    roundtrip(&Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::call("math::add", vec![Expr::u32(1), Expr::u32(2)]),
            ),
            Node::Return,
        ],
    ));
}
