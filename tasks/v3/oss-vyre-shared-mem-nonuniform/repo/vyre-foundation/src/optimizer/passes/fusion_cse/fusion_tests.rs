// Tests for `fusion.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.

use crate::ir::{BufferDecl, DataType, Expr, Ident, Node, Program};
use crate::optimizer::passes::fusion_cse::fusion::{
    collect_buffer_reads, collect_buffer_writes, Fusion,
};
use crate::optimizer::{PassScheduler, ProgramPassKind};

#[test]
fn preserves_happens_before_for_load_followed_by_write() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("state", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("snapshot", Expr::load("state", Expr::u32(0))),
            Node::store("state", Expr::u32(0), Expr::u32(7)),
            Node::store("out", Expr::u32(0), Expr::var("snapshot")),
        ],
    );

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("Fix: fusion must preserve happens-before ordering.");

    let body = match optimized.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fusion output must preserve the root region, got {entry:?}"),
    };

    assert!(matches!(
        body.as_slice(),
        [
            Node::Let {
                name,
                value: Expr::Load { buffer, .. }
            },
            Node::Store { buffer: state, .. },
            Node::Store {
                buffer: out,
                value: Expr::Var(snapshot),
                ..
            }
        ] if name == "snapshot"
            && buffer == "state"
            && state == "state"
            && out == "out"
            && snapshot == "snapshot"
    ));
}

#[test]
fn fusion_keeps_snapshot_before_later_state_write() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("state", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::store("state", Expr::u32(0), Expr::u32(5)),
            Node::let_bind("snapshot", Expr::load("state", Expr::u32(0))),
            Node::store("state", Expr::u32(0), Expr::u32(9)),
            Node::store("out", Expr::u32(0), Expr::var("snapshot")),
        ],
    );

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("Fix: fusion must preserve happens-before ordering.");

    let body = match optimized.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fusion output must preserve the root region, got {entry:?}"),
    };

    assert!(
        matches!(
            body.as_slice(),
            [
                Node::Store {
                    buffer: initial_state,
                    value: Expr::LitU32(5),
                    ..
                },
                Node::Let {
                    name,
                    value: Expr::Load { buffer: snapshot_source, .. }
                },
                Node::Store {
                    buffer: later_state,
                    value: Expr::LitU32(9),
                    ..
                },
                Node::Store {
                    buffer: out,
                    value: Expr::Var(snapshot),
                    ..
                }
            ] if initial_state == "state"
                && name == "snapshot"
                && snapshot_source == "state"
                && later_state == "state"
                && out == "out"
                && snapshot == "snapshot"
        ),
        "Fix: fusion must not move the snapshot load after the later state write."
    );
}

#[test]
fn buffer_write_flushes_only_dependent_pending_replacements() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("a", 0, DataType::U32).with_count(1),
            BufferDecl::read_write("b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("a_snap", Expr::load("a", Expr::u32(0))),
            Node::let_bind("b_snap", Expr::load("b", Expr::u32(0))),
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::store("out", Expr::u32(0), Expr::var("a_snap")),
            Node::store("out", Expr::u32(1), Expr::var("b_snap")),
        ],
    );

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("Fix: fusion must flush pending replacements by indexed buffer dependency.");

    let body = match optimized.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => panic!("Fix: fusion output must preserve the root region, got {entry:?}"),
    };

    assert!(
        matches!(
            body.as_slice(),
            [
                Node::Let {
                    name,
                    value: Expr::Load { buffer: a_load, .. },
                },
                Node::Store { buffer: a_store, .. },
                Node::Store {
                    buffer: out0,
                    value: Expr::Var(a_ref),
                    ..
                },
                Node::Store {
                    buffer: out1,
                    value: Expr::Load { buffer: b_load, .. },
                    ..
                },
            ] if name == "a_snap"
                && a_load == "a"
                && a_store == "a"
                && out0 == "out"
                && a_ref == "a_snap"
                && out1 == "out"
                && b_load == "b"
        ),
        "Fix: writing `a` must not flush the unrelated pending `b` load."
    );
}

#[test]
fn fuses_sequential_regions_with_low_pressure() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("tmp", 0, DataType::U32).with_count(32),
            BufferDecl::output("out", 1, DataType::U32).with_count(32),
        ],
        [1, 1, 1],
        vec![
            Node::Region {
                generator: "R1".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::store("tmp", Expr::u32(0), Expr::u32(1))]),
            },
            Node::Region {
                generator: "R2".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("tmp", Expr::u32(0)),
                )]),
            },
        ],
    );

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("Fix: fusion of sequential regions must succeed.");

    let entry = optimized.entry();
    assert_eq!(
        entry.len(),
        1,
        "Expected sequential regions to be fused, got: {:?}",
        entry
    );
    if let Node::Region {
        generator, body, ..
    } = &entry[0]
    {
        assert!(generator.contains("+"), "Generator must reflect fusion");
        assert_eq!(
            body.len(),
            2,
            "Fused body must contain nodes from both regions"
        );
    } else {
        panic!("Expected fused Region, got {:?}", entry[0]);
    }
}

#[test]
fn does_not_fuse_regions_with_high_pressure() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("large_tmp", 0, DataType::U32).with_count(2048),
            BufferDecl::output("out", 1, DataType::U32).with_count(32),
        ],
        [1, 1, 1],
        vec![
            Node::Region {
                generator: "R1".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::store(
                    "large_tmp",
                    Expr::u32(0),
                    Expr::u32(1),
                )]),
            },
            Node::Region {
                generator: "R2".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("large_tmp", Expr::u32(0)),
                )]),
            },
        ],
    );

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("Fix: fusion scheduler must handle high-pressure regions correctly.");

    let entry = optimized.entry();
    assert_eq!(
        entry.len(),
        2,
        "Expected sequential regions NOT to be fused due to high pressure, got: {:?}",
        entry
    );
}

#[test]
fn fusion_dependency_sets_include_async_and_indirect_nodes() {
    let nodes = vec![
        Node::async_load_ext(
            Ident::from("src"),
            Ident::from("dst"),
            Expr::load("offsets", Expr::u32(0)),
            Expr::var("size"),
            Ident::from("copy"),
        ),
        Node::async_store(
            Ident::from("dst"),
            Ident::from("sink"),
            Expr::buf_len("offsets"),
            Expr::u32(4),
            Ident::from("copy"),
        ),
        Node::IndirectDispatch {
            count_buffer: Ident::from("counts"),
            count_offset: 0,
        },
        Node::Trap {
            address: Box::new(Expr::load("trap_addr", Expr::u32(0))),
            tag: Ident::from("trap"),
        },
    ];

    let writes = collect_buffer_writes(&nodes);
    let reads = collect_buffer_reads(&nodes);

    assert!(writes.contains(&Ident::from("dst")));
    assert!(writes.contains(&Ident::from("sink")));
    for name in ["src", "dst", "offsets", "counts", "trap_addr"] {
        assert!(
            reads.contains(&Ident::from(name)),
            "missing async/indirect/trap read dependency `{name}`"
        );
    }
}

#[test]
fn walker_matches_canonical_on_corpus() {
    fn collect_buffer_writes_old(
        nodes: &[Node],
        visited: &mut Vec<Node>,
    ) -> rustc_hash::FxHashSet<Ident> {
        let mut writes = rustc_hash::FxHashSet::default();
        let mut stack: smallvec::SmallVec<[&Node; 64]> = nodes.iter().rev().collect();
        while let Some(node) = stack.pop() {
            visited.push(node.clone());
            match node {
                Node::Store { buffer, .. } => {
                    writes.insert(buffer.clone());
                }
                Node::AsyncLoad { destination, .. } | Node::AsyncStore { destination, .. } => {
                    writes.insert(destination.clone());
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    stack.extend(then.iter().rev());
                    stack.extend(otherwise.iter().rev());
                }
                Node::Loop { body, .. } | Node::Block(body) => {
                    stack.extend(body.iter().rev());
                }
                Node::Region { body, .. } => {
                    stack.extend(body.iter().rev());
                }
                _ => {}
            }
        }
        writes
    }

    let nodes = vec![
        Node::Region {
            generator: "R1".into(),
            source_region: None,
            body: std::sync::Arc::new(vec![
                Node::if_then(
                    Expr::bool(true),
                    vec![Node::store("buf_a", Expr::u32(0), Expr::u32(1))],
                ),
                Node::Block(vec![Node::store("buf_b", Expr::u32(0), Expr::u32(2))]),
            ]),
        },
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(10),
            vec![Node::store("buf_c", Expr::u32(0), Expr::u32(3))],
        ),
    ];

    let mut visited_old = Vec::new();
    let writes_old = collect_buffer_writes_old(&nodes, &mut visited_old);

    let mut visited_new = Vec::new();
    let mut writes_new = rustc_hash::FxHashSet::default();
    for node in &nodes {
        let _ = crate::visit::node_map::any_descendant(node, &mut |n| {
            visited_new.push(n.clone());
            match n {
                Node::Store { buffer, .. } => {
                    writes_new.insert(buffer.clone());
                }
                Node::AsyncLoad { destination, .. } | Node::AsyncStore { destination, .. } => {
                    writes_new.insert(destination.clone());
                }
                _ => {}
            }
            false
        });
    }

    assert_eq!(writes_old, writes_new, "Writes sets must match");
    assert_eq!(
        visited_old.len(),
        visited_new.len(),
        "Node set length must match"
    );

    for node in &visited_old {
        assert!(
            visited_new.contains(node),
            "Old walker visited a node that the new canonical walker missed"
        );
    }
    for node in &visited_new {
        assert!(
            visited_old.contains(node),
            "New canonical walker visited a node that the old walker missed"
        );
    }
}
