use super::stats::{
    NODE_KIND_LET, NODE_KIND_LOOP, NODE_KIND_REGION, NODE_KIND_RETURN, NODE_KIND_STORE,
};
use super::{Program, ProgramStats};
use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

#[test]
fn stats_matches_old_multi_walk_empty() {
    let program = Program::empty();
    let stats = program.stats();
    assert_eq!(
        *stats,
        ProgramStats {
            node_count: 1, // root region
            region_count: 1,
            call_count: 0,
            opaque_count: 0,
            top_level_regions: 1,
            static_storage_bytes: 0,
            capability_bits: 0,
            node_kinds_present: NODE_KIND_REGION,
            ..ProgramStats::default()
        }
    );
}

#[test]
fn stats_matches_old_multi_walk_single_store() {
    let program = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
    );
    let stats = program.stats();
    assert_eq!(
        *stats,
        ProgramStats {
            node_count: 3, // Region + Store + Return
            region_count: 1,
            call_count: 0,
            opaque_count: 0,
            top_level_regions: 1,
            static_storage_bytes: 4,
            instruction_count: 2,
            memory_op_count: 1,
            control_flow_count: 1,
            capability_bits: 0,
            node_kinds_present: NODE_KIND_REGION | NODE_KIND_STORE | NODE_KIND_RETURN,
            ..ProgramStats::default()
        }
    );
}

#[test]
fn stats_matches_old_multi_walk_batch() {
    let program = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::store("out", Expr::u32(1), Expr::u32(2)),
            Node::store("out", Expr::u32(2), Expr::u32(3)),
            Node::Return,
        ],
    );
    let stats = program.stats();
    assert_eq!(
        *stats,
        ProgramStats {
            node_count: 5, // Region + 3 Store + Return
            region_count: 1,
            call_count: 0,
            opaque_count: 0,
            top_level_regions: 1,
            static_storage_bytes: 16,
            instruction_count: 4,
            memory_op_count: 3,
            control_flow_count: 1,
            capability_bits: 0,
            node_kinds_present: NODE_KIND_REGION | NODE_KIND_STORE | NODE_KIND_RETURN,
            ..ProgramStats::default()
        }
    );
}

#[test]
fn stats_matches_old_multi_walk_region_chain() {
    #[allow(deprecated)]
    let program = Program::new(
        vec![],
        [1, 1, 1],
        vec![
            Node::Region {
                generator: "a".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![]),
            },
            Node::Region {
                generator: "b".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![]),
            },
        ],
    );
    let stats = program.stats();
    assert_eq!(
        *stats,
        ProgramStats {
            node_count: 2, // two top-level regions
            region_count: 2,
            call_count: 0,
            opaque_count: 0,
            top_level_regions: 2,
            static_storage_bytes: 0,
            capability_bits: 0,
            node_kinds_present: NODE_KIND_REGION,
            ..ProgramStats::default()
        }
    );
}

#[test]
fn stats_matches_old_multi_walk_recursive() {
    let program = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(10),
            vec![Node::let_bind("x", Expr::call("foo", vec![Expr::u32(1)]))],
        )],
    );
    let stats = program.stats();
    assert_eq!(
        *stats,
        ProgramStats {
            node_count: 3, // Region + Loop + Let
            region_count: 1,
            call_count: 1,
            opaque_count: 0,
            top_level_regions: 1,
            static_storage_bytes: 4,
            instruction_count: 3,
            control_flow_count: 1,
            register_pressure_estimate: 1,
            capability_bits: 0,
            node_kinds_present: NODE_KIND_REGION | NODE_KIND_LOOP | NODE_KIND_LET,
            ..ProgramStats::default()
        }
    );
}

#[test]
fn stats_cache_hit_returns_same_reference() {
    let program = Program::empty();
    let s1 = program.stats();
    let s2 = program.stats();
    assert!(
        std::ptr::eq(s1, s2),
        "Fix: repeated stats() calls must return cached reference"
    );
}

/// `ProgramStats::node_kinds_present` must use the same bit positions
/// as `optimizer::program_soa::NodeKind` so a pass can read either
/// source-of-truth and get the same answer. Without this contract,
/// passes that gate on `program.stats().has_node_kind_*` and passes
/// that gate on `ProgramFacts::has_kind` would silently disagree.
#[test]
fn node_kinds_present_bit_positions_match_program_soa_node_kind() {
    use crate::optimizer::program_soa::{kind_mask, NodeKind};
    assert_eq!(NODE_KIND_LET, kind_mask(NodeKind::Let));
    assert_eq!(
        super::stats::NODE_KIND_ASSIGN,
        kind_mask(NodeKind::Assign)
    );
    assert_eq!(NODE_KIND_STORE, kind_mask(NodeKind::Store));
    assert_eq!(super::stats::NODE_KIND_IF, kind_mask(NodeKind::If));
    assert_eq!(NODE_KIND_LOOP, kind_mask(NodeKind::Loop));
    assert_eq!(
        super::stats::NODE_KIND_INDIRECT_DISPATCH,
        kind_mask(NodeKind::IndirectDispatch)
    );
    assert_eq!(
        super::stats::NODE_KIND_ASYNC_LOAD,
        kind_mask(NodeKind::AsyncLoad)
    );
    assert_eq!(
        super::stats::NODE_KIND_ASYNC_STORE,
        kind_mask(NodeKind::AsyncStore)
    );
    assert_eq!(
        super::stats::NODE_KIND_ASYNC_WAIT,
        kind_mask(NodeKind::AsyncWait)
    );
    assert_eq!(super::stats::NODE_KIND_TRAP, kind_mask(NodeKind::Trap));
    assert_eq!(
        super::stats::NODE_KIND_RESUME,
        kind_mask(NodeKind::Resume)
    );
    assert_eq!(NODE_KIND_RETURN, kind_mask(NodeKind::Return));
    assert_eq!(
        super::stats::NODE_KIND_BARRIER,
        kind_mask(NodeKind::Barrier)
    );
    assert_eq!(
        super::stats::NODE_KIND_BLOCK,
        kind_mask(NodeKind::Block)
    );
    assert_eq!(NODE_KIND_REGION, kind_mask(NodeKind::Region));
    assert_eq!(
        super::stats::NODE_KIND_ALL_REDUCE,
        kind_mask(NodeKind::AllReduce)
    );
    assert_eq!(
        super::stats::NODE_KIND_ALL_GATHER,
        kind_mask(NodeKind::AllGather)
    );
    assert_eq!(
        super::stats::NODE_KIND_REDUCE_SCATTER,
        kind_mask(NodeKind::ReduceScatter)
    );
    assert_eq!(
        super::stats::NODE_KIND_BROADCAST,
        kind_mask(NodeKind::Broadcast)
    );
    assert_eq!(
        super::stats::NODE_KIND_OPAQUE,
        kind_mask(NodeKind::Opaque)
    );
}
