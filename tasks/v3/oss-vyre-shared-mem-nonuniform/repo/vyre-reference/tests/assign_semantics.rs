//! Enforces the `docs/ir-semantics.md` statement-IR contract for
//! `Node::Let` + `Node::Assign`. Every rule in the doc has a test.
//!
//! These tests are the load-bearing source of truth  -  a refactor
//! that violates the contract fails here, not silently in a Cat-A
//! composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::value::Value;

fn run(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let values: Vec<Value> = inputs.into_iter().map(Value::from).collect();
    let outputs = vyre_reference::reference_eval(program, &values).expect("program must execute");
    outputs.into_iter().map(|v| v.to_bytes()).collect()
}

fn store_u32_program(body: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        body,
    )
}

#[test]
fn assign_mutates_prior_let_in_same_scope() {
    let body = vec![
        Node::let_bind("acc", Expr::u32(10)),
        Node::assign("acc", Expr::add(Expr::var("acc"), Expr::u32(5))),
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let out = run(&store_u32_program(body), vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(out[0][..4].try_into().unwrap());
    assert_eq!(val, 15, "assign must mutate the prior let binding");
}

#[test]
fn assign_accumulates_across_loop_iterations() {
    // acc starts at 0; loop 4 times, add i each time. Expected: 0+1+2+3 = 6.
    let body = vec![
        Node::let_bind("acc", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(4),
            vec![Node::assign(
                "acc",
                Expr::add(Expr::var("acc"), Expr::var("i")),
            )],
        ),
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let out = run(&store_u32_program(body), vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(out[0][..4].try_into().unwrap());
    assert_eq!(val, 6, "loop-scope assign must accumulate into outer let");
}

#[test]
fn assign_from_inside_region_mutates_outer_let() {
    // The Region is semantically transparent  -  an Assign inside it
    // must mutate the outer binding.
    let inner = vec![Node::assign(
        "acc",
        Expr::add(Expr::var("acc"), Expr::u32(42)),
    )];
    let region = Node::Region {
        generator: "test".into(),
        source_region: None,
        body: std::sync::Arc::new(inner),
    };
    let body = vec![
        Node::let_bind("acc", Expr::u32(100)),
        region,
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let out = run(&store_u32_program(body), vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(out[0][..4].try_into().unwrap());
    assert_eq!(
        val, 142,
        "Region transparency: outer let must observe inner assign"
    );
}

#[test]
fn shadowing_is_rejected_by_validator_v008() {
    // Per docs/ir-semantics.md: shadowing a prior Let anywhere in the
    // visible scope chain is V008. The validator enforces this so
    // SSA-conversion + autodiff + canonical-form passes can assume
    // every Var(name) resolves to one Let(name, …) site.
    let inner = vec![
        Node::let_bind("acc", Expr::u32(99)),
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let body = vec![Node::let_bind("acc", Expr::u32(1)), Node::Block(inner)];
    let program = store_u32_program(body);
    let errors = vyre_foundation::validate::validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("duplicate local binding")),
        "shadowing must fail validation (V008), got {:?}",
        errors.iter().map(|e| e.message()).collect::<Vec<_>>()
    );
}

#[test]
fn assign_survives_nested_block_scope() {
    // Outer let acc=0. Inner block mutates it. Outer then stores.
    // The mutation must survive the inner scope's close because
    // Block scope owns fresh Lets but not pre-existing bindings.
    let inner = vec![Node::assign(
        "acc",
        Expr::add(Expr::var("acc"), Expr::u32(7)),
    )];
    let body = vec![
        Node::let_bind("acc", Expr::u32(100)),
        Node::Block(inner),
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let out = run(&store_u32_program(body), vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(out[0][..4].try_into().unwrap());
    assert_eq!(val, 107, "assigns to outer let persist across block scope");
}
