//! RFC-0004 collective IR node contracts.

use vyre_foundation::ir::stats::{
    NODE_KIND_ALL_GATHER, NODE_KIND_ALL_REDUCE, NODE_KIND_BROADCAST, NODE_KIND_REDUCE_SCATTER,
};
use vyre_foundation::ir::{
    BufferAccess, BufferDecl, CollectiveOp, CommGroup, DataType, Node, Program,
};
use vyre_foundation::program_caps::scan;
use vyre_foundation::validate::{
    validate, validate_with_options, BackendCapabilities, ValidationOptions,
};

fn collective_buffers() -> Vec<BufferDecl> {
    vec![
        BufferDecl::storage("a", 0, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        BufferDecl::read("input", 1, DataType::U32).with_count(8),
        BufferDecl::storage("out", 2, BufferAccess::ReadWrite, DataType::U32).with_count(8),
    ]
}

fn collective_program() -> Program {
    Program::wrapped(
        collective_buffers(),
        [64, 1, 1],
        vec![
            Node::AllReduce {
                buffer: "a".into(),
                op: CollectiveOp::Sum,
                group: CommGroup::WORLD,
            },
            Node::AllGather {
                input: "input".into(),
                output: "out".into(),
                group: CommGroup::WORLD,
            },
            Node::ReduceScatter {
                input: "input".into(),
                output: "out".into(),
                op: CollectiveOp::Max,
                group: CommGroup(3),
            },
            Node::Broadcast {
                buffer: "a".into(),
                root: 1,
                group: CommGroup(3),
            },
        ],
    )
}

fn collective_options() -> ValidationOptions<'static> {
    ValidationOptions::default().with_backend_capabilities(BackendCapabilities {
        supports_distributed_collectives: true,
        ..BackendCapabilities::default()
    })
}

#[test]
fn collective_nodes_require_explicit_backend_capability() {
    let errors = validate(&collective_program());
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("V046: distributed collective nodes require backend collective support")),
        "Fix: collectives must not silently validate on scalar or single-device backends: {errors:?}"
    );
}

#[test]
fn collective_nodes_validate_when_backend_declares_support() {
    let report = validate_with_options(&collective_program(), collective_options());
    assert!(
        report.errors.is_empty(),
        "Fix: supported RFC-0004 collectives must validate cleanly, got {:?}",
        report.errors
    );
}

#[test]
fn split_collectives_reject_element_type_mismatch() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(8),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::F32).with_count(8),
        ],
        [64, 1, 1],
        vec![Node::AllGather {
            input: "input".into(),
            output: "out".into(),
            group: CommGroup::WORLD,
        }],
    );

    let report = validate_with_options(&program, collective_options());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.message().contains("element")),
        "Fix: split collectives must reject mismatched input/output element types: {:?}",
        report.errors
    );
}

#[test]
fn collective_nodes_roundtrip_through_program_wire() {
    let program = collective_program();
    let wire = program
        .to_wire()
        .expect("Fix: collective program must encode.");
    let decoded = Program::from_wire(&wire).expect("Fix: collective program must decode.");
    let nodes = flatten_nodes(decoded.entry());

    assert_eq!(nodes.len(), 5);
    assert!(matches!(
        nodes[1],
        Node::AllReduce {
            op: CollectiveOp::Sum,
            group: CommGroup(0),
            ..
        }
    ));
    assert!(matches!(
        nodes[2],
        Node::AllGather {
            group: CommGroup(0),
            ..
        }
    ));
    assert!(matches!(
        nodes[3],
        Node::ReduceScatter {
            op: CollectiveOp::Max,
            group: CommGroup(3),
            ..
        }
    ));
    assert!(matches!(
        nodes[4],
        Node::Broadcast {
            root: 1,
            group: CommGroup(3),
            ..
        }
    ));
}

fn flatten_nodes(nodes: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    for node in nodes {
        out.push(node);
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                out.extend(flatten_nodes(then));
                out.extend(flatten_nodes(otherwise));
            }
            Node::Loop { body, .. } | Node::Block(body) => out.extend(flatten_nodes(body)),
            Node::Region { body, .. } => out.extend(flatten_nodes(body)),
            _ => {}
        }
    }
    out
}

#[test]
fn collective_nodes_are_visible_to_program_stats() {
    let program = collective_program();
    let stats = program.stats();
    let mask = NODE_KIND_ALL_REDUCE
        | NODE_KIND_ALL_GATHER
        | NODE_KIND_REDUCE_SCATTER
        | NODE_KIND_BROADCAST;
    assert!(
        stats.has_any_node_kind(mask),
        "Fix: optimizer skip gates must see collective node kinds."
    );
    assert_eq!(
        stats.node_kinds_present & mask,
        mask,
        "Fix: every RFC-0004 collective node kind must have a stable ProgramStats bit."
    );
}

#[test]
fn collective_nodes_are_visible_to_required_capability_scan() {
    let required = scan(&collective_program());
    assert!(
        required.distributed_collectives,
        "Fix: dispatch admission must see RFC-0004 collective requirements before backend launch."
    );
}
