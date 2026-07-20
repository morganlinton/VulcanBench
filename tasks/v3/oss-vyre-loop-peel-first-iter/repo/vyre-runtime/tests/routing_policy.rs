//! Runtime routing contract tests.

use vyre_foundation::execution_plan::plan;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_runtime::routing::standard_policy::StandardPolicy;
use vyre_runtime::routing::{RoutingDecision, RoutingEngine};

fn program_with_nodes(node_count: u32, output_count: u32) -> Program {
    let body = (0..node_count)
        .map(|idx| Node::store("out", Expr::u32(idx), Expr::u32(idx)))
        .collect::<Vec<_>>();
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(output_count)],
        [128, 1, 1],
        body,
    )
}

#[test]
fn standard_runtime_routing_is_megakernel_first() {
    let engine = RoutingEngine::new(StandardPolicy);
    for (node_count, output_count) in [(64, 64), (64, 16_384), (65, 65), (1025, 1025)] {
        let plan = plan(&program_with_nodes(node_count, output_count))
            .expect("routing fixture must be canonical");
        assert_eq!(engine.route(&plan), RoutingDecision::PersistentMegakernel);
    }
}

#[test]
fn standard_runtime_routing_never_selects_reference_route_implicitly() {
    let engine = RoutingEngine::new(StandardPolicy);
    let plan = plan(&program_with_nodes(1, 1)).expect("routing fixture must be canonical");
    assert_ne!(engine.route(&plan), RoutingDecision::CpuSimd);
}
