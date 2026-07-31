//! Regression tests for Naga lowering of Region and loop scope behavior.

use naga::{Block, Statement};
use std::sync::Arc;
use vyre_driver::DispatchConfig;
use vyre_emit_naga::program::emit_module;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

fn emit_validated_module_and_wgsl(program: &Program) -> (naga::Module, String) {
    let module = emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Fix: test program must lower to a valid Naga module.");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Fix: lowered module must validate before WGSL serialization.");
    let wgsl =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .expect("Fix: lowered module must serialize to WGSL.");
    (module, wgsl)
}

fn count_blocks(block: &Block) -> usize {
    block
        .iter()
        .map(|statement| match statement {
            Statement::Block(child) => 1 + count_blocks(child),
            Statement::If { accept, reject, .. } => count_blocks(accept) + count_blocks(reject),
            Statement::Loop {
                body, continuing, ..
            } => count_blocks(body) + count_blocks(continuing),
            Statement::Switch { cases, .. } => {
                cases.iter().map(|case| count_blocks(&case.body)).sum()
            }
            _ => 0,
        })
        .sum()
}

fn previous_line_before<'a>(wgsl: &'a str, needle: &str) -> Option<&'a str> {
    let mut previous = None;
    for line in wgsl.lines() {
        let trimmed = line.trim();
        if trimmed.contains(needle) {
            return previous;
        }
        if !trimmed.is_empty() {
            previous = Some(trimmed);
        }
    }
    None
}

#[test]
fn large_region_lowers_to_real_naga_block_and_wgsl_scope() {
    let region_body = (0..65)
        .map(|index| Node::store("out", Expr::u32(index), Expr::u32(index + 1)))
        .collect::<Vec<_>>();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32).with_count(65)],
        [1, 1, 1],
        vec![Node::Region {
            generator: "naga_loop_region_followup::large_region".into(),
            source_region: None,
            body: Arc::new(region_body),
        }],
    );

    let (module, wgsl) = emit_validated_module_and_wgsl(&program);
    let entry = module
        .entry_points
        .first()
        .expect("Fix: Naga module must contain the compute entry point.");

    assert!(
        count_blocks(&entry.function.body) >= 1,
        "Fix: non-inlined Node::Region must lower as a real Naga block, not disappear before statement lowering. block_count={}\n{wgsl}",
        count_blocks(&entry.function.body),
    );
    assert!(
        wgsl.contains(") {\n    {\n"),
        "Fix: Region block boundaries must survive WGSL emission as lexical scopes.\n{wgsl}",
    );
    assert!(
        wgsl.contains("out[64u] = 65u;"),
        "Fix: the full Region body must lower into the emitted block, including the tail statement.\n{wgsl}",
    );
}

#[test]
fn loop_initial_bound_side_effect_is_evaluated_once() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("counter", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::atomic_add("counter", Expr::u32(0), Expr::u32(1)),
                Expr::u32(3),
                vec![Node::store("out", Expr::u32(0), Expr::var("i"))],
            ),
            Node::Return,
        ],
    );

    let (_, wgsl) = emit_validated_module_and_wgsl(&program);
    assert_eq!(
        wgsl.matches("atomicAdd").count(),
        1,
        "Fix: loop initial bounds with side effects must initialize the loop local once, not re-emit in guard or continuing blocks.\n{wgsl}",
    );
}

/// Region phi-merge end-to-end (WGSL-level): a `Node::Region` inside a
/// `Node::Loop` whose body Assigns a loop-carried name must publish the
/// in-region final value back through the named-carrier function-local
/// the Loop allocated. The named-carrier round-trip is the architectural
/// fix that unblocks the GPU lex `n_tokens=0` symptom on real C input.
///
/// Uses an input-dependent value (loaded from `seed` buffer) for the
/// loop bound and the increment so the optimizer cannot constant-fold
/// the loop body  -  we want the actual carrier mechanism in the shader.
#[test]
fn region_inside_loop_publishes_carrier_through_named_local() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("seed", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::load("seed", Expr::u32(0)),
                vec![Node::Region {
                    generator: "naga_loop_region_followup::step".into(),
                    source_region: None,
                    body: Arc::new(vec![Node::assign(
                        "acc",
                        Expr::add(Expr::var("acc"), Expr::load("seed", Expr::u32(0))),
                    )]),
                }],
            ),
            Node::store("out", Expr::u32(0), Expr::var("acc")),
            Node::Return,
        ],
    );

    let (_, wgsl) = emit_validated_module_and_wgsl(&program);

    // Function-local for the named carrier `acc` must be emitted  -
    // proves the named-carrier slot path fired (Region+Loop share the
    // same `vyre_named_carry_acc` local). Without the Region phi-merge
    // fix, in-region Assigns to `acc` would never reach the post-loop
    // reader: the inner Region's lower_child_node previously rebound
    // scope without carrier round-trip, so the post-loop reader saw
    // the pre-loop seed instead of the in-region final value.
    assert!(
        wgsl.contains("vyre_named_carry_acc"),
        "Fix: region inside loop must allocate the named-carrier local for `acc`.\n{wgsl}"
    );

    // The Loop must survive (input-dependent bound prevents folding).
    assert!(
        wgsl.contains("loop {"),
        "Fix: input-dependent Loop must lower to a real WGSL loop.\n{wgsl}"
    );

    // Inside the loop body, there must be a Store to the named carrier  -
    // the LoopCarrierEnd that commits the per-iteration update pushed
    // by the active-carrier path of `Node::Assign` lowering. Without
    // the Region phi-merge, no such store exists for the inner Region's
    // Assign.
    assert!(
        wgsl.matches("vyre_named_carry_acc =").count() >= 1,
        "Fix: in-region Assign must emit a Store to the named-carrier local.\n{wgsl}"
    );
}

#[test]
fn loop_variable_shadowing_restores_outer_local_after_body_lowering() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32).with_count(2)],
        [1, 1, 1],
        vec![
            Node::let_bind("i", Expr::u32(99)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(1),
                vec![Node::store("out", Expr::u32(0), Expr::var("i"))],
            ),
            Node::store("out", Expr::u32(1), Expr::var("i")),
            Node::Return,
        ],
    );

    let (_, wgsl) = emit_validated_module_and_wgsl(&program);
    let loop_source = previous_line_before(&wgsl, "out[0u] =")
        .expect("Fix: loop-body store must be preceded by a local load temporary.");
    let outer_source = previous_line_before(&wgsl, "out[1u] =")
        .expect("Fix: post-loop store must be preceded by a local load temporary.");
    assert!(
        loop_source.ends_with("= i_1;"),
        "Fix: loop body must read the shadowing loop-local, not the outer `i`. previous_line={loop_source}\n{wgsl}",
    );
    assert!(
        outer_source.ends_with("= i;"),
        "Fix: after loop lowering, local lookup for `i` must be restored to the outer binding. previous_line={outer_source}\n{wgsl}",
    );
    let loop_store = wgsl
        .find("out[0u] =")
        .expect("Fix: loop-body store must be emitted.");
    let outer_store = wgsl
        .find("out[1u] =")
        .expect("Fix: post-loop store must be emitted.");
    assert!(
        loop_store < outer_store,
        "Fix: the loop-local use must occur before the post-loop outer-local use.\n{wgsl}",
    );
}
