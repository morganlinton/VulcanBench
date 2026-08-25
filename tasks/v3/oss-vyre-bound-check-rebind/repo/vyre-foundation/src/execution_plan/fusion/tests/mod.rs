use super::fuse::{fuse_programs, fuse_programs_vec, merge_programs_shared};
use super::FusionError;
use crate::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

#[test]
fn empty_batch_yields_empty_program() {
    let fused = fuse_programs(&[]).unwrap();
    assert!(fused.is_explicit_noop());
}

#[test]
fn single_program_passthrough() {
    let p = Program::wrapped(
        vec![BufferDecl::read("x", 0, DataType::U32)],
        [64, 1, 1],
        vec![Node::let_bind(
            "a",
            crate::ir::Expr::load("x", crate::ir::Expr::u32(0)),
        )],
    );
    let fused = fuse_programs(&[p.clone()]).unwrap();
    assert_eq!(fused.entry().len(), p.entry().len());
}

#[test]
fn single_program_vec_moves_without_clone() {
    let p = Program::wrapped(
        vec![BufferDecl::read("x", 0, DataType::U32)],
        [64, 1, 1],
        vec![Node::let_bind(
            "a",
            crate::ir::Expr::load("x", crate::ir::Expr::u32(0)),
        )],
    );
    let entry_len = p.entry().len();
    let fused = fuse_programs_vec(vec![p]).unwrap();
    assert_eq!(fused.entry().len(), entry_len);
}

#[test]
fn barrier_inserted_for_read_then_atomic() {
    let reader = Program::wrapped(
        vec![BufferDecl::read("state", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "snap",
            crate::ir::Expr::load("state", crate::ir::Expr::u32(0)),
        )],
    );
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "old",
            crate::ir::Expr::atomic_add("state", crate::ir::Expr::u32(0), crate::ir::Expr::u32(1)),
        )],
    );

    let fused = fuse_programs(&[reader, writer]).unwrap();

    // The combined entry should have a Barrier between the two arms.
    // Because the top-level entry contains non-Region nodes (Barrier),
    // Program::wrapped inserts a root Region.  We need to look inside it.
    let body = match fused.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fused entry must be wrapped in a root Region, got {entry:?}"),
    };
    let barrier_positions: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, Node::Barrier { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        barrier_positions.len(),
        1,
        "Fix: fusion must insert exactly one barrier between read and atomic arms"
    );
}

#[test]
fn divergent_invocation_gated_writer_upgrades_barrier_to_grid_sync() {
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        vec![Node::if_then(
            crate::ir::Expr::eq(crate::ir::Expr::gid_x(), crate::ir::Expr::u32(0)),
            vec![Node::store(
                "state",
                crate::ir::Expr::u32(0),
                crate::ir::Expr::u32(7),
            )],
        )],
    );
    let reader = Program::wrapped(
        vec![BufferDecl::read("state", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        vec![Node::let_bind(
            "snap",
            crate::ir::Expr::load("state", crate::ir::Expr::u32(0)),
        )],
    );

    let fused = fuse_programs(&[writer, reader]).unwrap();
    let body = match fused.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fused entry must be wrapped in a root Region, got {entry:?}"),
    };
    let has_grid_sync = body.iter().any(|node| {
        matches!(
            node,
            Node::Barrier {
                ordering: crate::memory_model::MemoryOrdering::GridSync,
                ..
            }
        )
    });
    assert!(
        has_grid_sync,
        "Fix: invocation-gated cross-arm writes must use GridSync, not a workgroup-only barrier"
    );
}

#[test]
fn launch_indexed_writer_upgrades_raw_barrier_to_grid_sync() {
    let lane = crate::ir::Expr::var("lane");
    let block = crate::ir::Expr::var("block");
    let global = crate::ir::Expr::var("global");
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(256)],
        [128, 1, 1],
        vec![
            Node::let_bind("lane", crate::ir::Expr::LocalId { axis: 0 }),
            Node::let_bind("block", crate::ir::Expr::WorkgroupId { axis: 0 }),
            Node::let_bind(
                "global",
                crate::ir::Expr::add(
                    crate::ir::Expr::mul(block.clone(), crate::ir::Expr::u32(128)),
                    lane.clone(),
                ),
            ),
            Node::store("state", global.clone(), global.clone()),
        ],
    );
    let reader = Program::wrapped(
        vec![BufferDecl::read("state", 0, DataType::U32).with_count(256)],
        [128, 1, 1],
        vec![
            Node::let_bind("lane", crate::ir::Expr::LocalId { axis: 0 }),
            Node::let_bind("block", crate::ir::Expr::WorkgroupId { axis: 0 }),
            Node::let_bind(
                "global",
                crate::ir::Expr::add(crate::ir::Expr::mul(block, crate::ir::Expr::u32(128)), lane),
            ),
            Node::let_bind("snap", crate::ir::Expr::load("state", global)),
        ],
    );

    let fused = fuse_programs(&[writer, reader]).unwrap();
    let body = match fused.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fused entry must be wrapped in a root Region, got {entry:?}"),
    };
    let has_grid_sync = body.iter().any(|node| {
        matches!(
            node,
            Node::Barrier {
                ordering: crate::memory_model::MemoryOrdering::GridSync,
                ..
            }
        )
    });

    assert!(
        has_grid_sync,
        "Fix: launch-indexed cross-arm writes must use GridSync so later arms cannot read another workgroup's stale output."
    );
}

#[test]
fn uniform_cross_arm_writer_uses_workgroup_barrier() {
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        vec![Node::store(
            "state",
            crate::ir::Expr::u32(0),
            crate::ir::Expr::u32(7),
        )],
    );
    let reader = Program::wrapped(
        vec![BufferDecl::read("state", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        vec![Node::let_bind(
            "snap",
            crate::ir::Expr::load("state", crate::ir::Expr::u32(0)),
        )],
    );

    let fused = fuse_programs(&[writer, reader]).unwrap();
    let body = match fused.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fused entry must be wrapped in a root Region, got {entry:?}"),
    };
    let has_workgroup_barrier = body.iter().any(|node| {
        matches!(
            node,
            Node::Barrier {
                ordering: crate::memory_model::MemoryOrdering::SeqCst,
                ..
            }
        )
    });
    let has_grid_sync = body.iter().any(|node| {
        matches!(
            node,
            Node::Barrier {
                ordering: crate::memory_model::MemoryOrdering::GridSync,
                ..
            }
        )
    });

    assert!(
        has_workgroup_barrier,
        "Fix: uniform cross-arm writes must still get a workgroup memory barrier"
    );
    assert!(
        !has_grid_sync,
        "Fix: fusion must not force a global kernel split for uniform cross-arm writes"
    );
}

#[test]
fn self_composing_parser_rejected() {
    let parser = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_entry_op_id("vyre-libs::parsing::test_parser")
    .with_non_composable_with_self(true);

    let result = fuse_programs(&[parser.clone(), parser]);
    assert!(
        matches!(result, Err(FusionError::SelfAliasing(_))),
        "Fix: fusing two copies of a non-composable parser must fail"
    );
}

#[test]
fn duplicate_buffer_dedup_upgrades_access() {
    let a = Program::wrapped(
        vec![BufferDecl::read("x", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let b = Program::wrapped(
        vec![BufferDecl::read_write("x", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let fused = fuse_programs(&[a, b]).unwrap();
    assert_eq!(fused.buffers().len(), 1);
    assert_eq!(fused.buffers()[0].access(), BufferAccess::ReadWrite);
}

#[test]
fn multi_arm_regions_flatten_into_one_executable_body() {
    let a = Program::wrapped(
        vec![BufferDecl::output("a_out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "a_out",
            crate::ir::Expr::u32(0),
            crate::ir::Expr::u32(1),
        )],
    );
    let b = Program::wrapped(
        vec![BufferDecl::output("b_out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "b_out",
            crate::ir::Expr::u32(0),
            crate::ir::Expr::u32(2),
        )],
    );

    let fused = fuse_programs(&[a, b]).unwrap();
    let body = match fused.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fused multi-arm programs must have one root Region, got {entry:?}"),
    };
    let stores = body.iter().map(count_stores).sum::<usize>();
    assert_eq!(
        stores, 2,
        "Fix: fusion must flatten top-level arm Regions into executable arm blocks"
    );
}

/// A value produced in one arm (`let __cmp_5 = load(flag,0)`) and consumed in
/// another (`store(out,0, Var(__cmp_5))`) must keep ONE name and ONE scope
/// after an intra-rule shared-namespace merge, so the merged program validates
/// clean. This is the `csrf_missing_token` "reference to undeclared variable"
/// miscompile distilled to its core: isolated fusion alpha-renames the two
/// arms apart, breaking the decl→use link; shared merge keeps it.
#[test]
fn shared_merge_keeps_cross_arm_value_linked_and_valid() {
    let producer = Program::wrapped(
        vec![BufferDecl::read_write("flag", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "__cmp_5",
            crate::ir::Expr::load("flag", crate::ir::Expr::u32(0)),
        )],
    );
    let consumer = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            crate::ir::Expr::u32(0),
            crate::ir::Expr::var("__cmp_5"),
        )],
    );

    let merged = merge_programs_shared(&[producer.clone(), consumer.clone()])
        .expect("shared merge of producer+consumer composes");
    let errors = crate::validate::validate(&merged);
    let undeclared: Vec<_> = errors
        .iter()
        .filter(|e| e.message().contains("undeclared variable"))
        .collect();
    assert!(
        undeclared.is_empty(),
        "shared merge must keep `__cmp_5` declared-then-used in one scope; got {:?}",
        undeclared.iter().map(|e| e.message()).collect::<Vec<_>>()
    );

    // Contrast: isolated fusion alpha-renames the arms apart, so the consumer's
    // `Var(__cmp_5)` becomes `__vyre_fuse_a1_…` with no surviving decl — the
    // exact miscompile shared merge exists to avoid. This pins WHY the shared
    // path is required for intra-rule composition.
    let isolated = fuse_programs(&[producer, consumer]).expect("isolated fusion composes");
    let isolated_undeclared = crate::validate::validate(&isolated)
        .into_iter()
        .any(|e| e.message().contains("undeclared variable"));
    assert!(
        isolated_undeclared,
        "isolated fusion is expected to desync the cross-arm value (renames arms apart); \
         if this no longer holds, the shared-vs-isolated distinction has changed"
    );
}

fn count_stores(node: &Node) -> usize {
    match node {
        Node::Store { .. } => 1,
        Node::Block(nodes) => nodes.iter().map(count_stores).sum(),
        Node::Region { body, .. } => body.iter().map(count_stores).sum(),
        Node::If {
            then, otherwise, ..
        } => then.iter().chain(otherwise.iter()).map(count_stores).sum(),
        Node::Loop { body, .. } => body.iter().map(count_stores).sum(),
        _ => 0,
    }
}
