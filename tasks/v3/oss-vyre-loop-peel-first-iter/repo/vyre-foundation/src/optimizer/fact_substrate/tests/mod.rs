use super::*;
use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

#[test]
fn derive_use_counts_simple() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("y", Expr::add(Expr::var("x"), Expr::var("x"))),
            Node::store("out", Expr::u32(0), Expr::var("y")),
        ],
    );
    let substrate = FactSubstrate::derive(&program);
    assert_eq!(substrate.use_count_of(&Ident::from("x")), 2);
    assert_eq!(substrate.use_count_of(&Ident::from("y")), 1);
    assert_eq!(substrate.use_count_of(&Ident::from("z")), 0);
}

#[test]
fn derive_use_counts_async_operands() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::read_write("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("offset", Expr::u32(1)),
            Node::let_bind("size", Expr::u32(2)),
            Node::async_load_ext(
                Ident::from("input"),
                Ident::from("out"),
                Expr::var("offset"),
                Expr::var("size"),
                Ident::from("copy"),
            ),
        ],
    );
    let substrate = FactSubstrate::derive(&program);
    assert_eq!(substrate.use_count_of(&Ident::from("offset")), 1);
    assert_eq!(substrate.use_count_of(&Ident::from("size")), 1);
}

#[test]
fn derive_use_facts_records_buffer_accesses_and_index_axes() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(64),
            BufferDecl::read_write("out", 1, DataType::U32).with_count(64),
        ],
        [8, 8, 1],
        vec![Node::store(
            "out",
            Expr::gid_y(),
            Expr::load("input", Expr::gid_x()),
        )],
    );

    let substrate = FactSubstrate::derive_use_only(&program);
    assert!(substrate.has_fresh_use_facts_for(&program));
    assert!(!substrate.is_fresh_for(&program));
    let facts = substrate.use_facts().unwrap();
    assert_eq!(facts.buffer_reads.get(&Ident::from("input")), Some(&1));
    assert_eq!(facts.buffer_writes.get(&Ident::from("out")), Some(&1));
    assert_eq!(facts.dominant_index_axis(&Ident::from("input")), Some(0));
    assert_eq!(facts.dominant_index_axis(&Ident::from("out")), Some(1));
}

#[test]
fn derive_use_facts_records_scalar_mediated_buffer_dependencies() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(1),
            BufferDecl::read_write("scratch", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::store("scratch", Expr::u32(0), Expr::var("x")),
            Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
        ],
    );

    let substrate = FactSubstrate::derive_use_only(&program);
    let facts = substrate.use_facts().unwrap();
    assert!(facts
        .var_buffer_deps
        .get(&Ident::from("x"))
        .is_some_and(|deps| deps.contains(&Ident::from("input"))));
    assert!(facts
        .buffer_write_deps
        .get(&Ident::from("scratch"))
        .is_some_and(|deps| deps.contains(&Ident::from("input"))));
    assert!(facts
        .buffer_write_deps
        .get(&Ident::from("out"))
        .is_some_and(|deps| deps.contains(&Ident::from("scratch"))));
}

#[test]
fn derive_use_facts_records_indirect_dispatch_count_buffers() {
    let program = Program::wrapped(
        vec![BufferDecl::read("counts", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::indirect_dispatch("counts", 0)],
    );

    let substrate = FactSubstrate::derive_use_only(&program);
    let facts = substrate.use_facts().unwrap();
    assert!(facts
        .indirect_dispatch_buffers
        .contains(&Ident::from("counts")));
    assert_eq!(facts.buffer_reads.get(&Ident::from("counts")), Some(&1));
}

#[test]
fn derive_type_facts_float_propagation() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::f32(1.0)),
            Node::let_bind("b", Expr::add(Expr::var("a"), Expr::f32(2.0))),
        ],
    );
    let substrate = FactSubstrate::derive(&program);
    let types = substrate.type_map.as_ref().unwrap();
    assert_eq!(types.var_types.get(&Ident::from("a")), Some(&DataType::F32));
    assert_eq!(types.var_types.get(&Ident::from("b")), Some(&DataType::F32));
}

#[test]
fn derive_type_facts_records_loads_and_expression_types() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(1),
            BufferDecl::read_write("out", 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    );

    let substrate = FactSubstrate::derive(&program);
    let types = substrate.type_map.as_ref().unwrap();
    assert_eq!(types.var_types.get(&Ident::from("x")), Some(&DataType::F32));
    assert!(
        !types.expr_types.is_empty(),
        "FactSubstrate::TypeFacts promises expression type facts; derive() must populate them"
    );
}

#[test]
fn invalidate_clears_all() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let mut substrate = FactSubstrate::derive(&program);
    assert!(substrate.is_fresh_for(&program));
    substrate.invalidate();
    assert!(!substrate.is_fresh_for(&program));
    assert!(substrate.shape.is_none());
}

#[test]
fn typed_partitions_report_freshness_by_backing_fact_family() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    let full = FactSubstrate::derive(&program);
    let full_partitions = full.fresh_partitions_for(&program);
    for partition in FactPartition::ALL {
        assert!(
            full_partitions.contains(&partition),
            "derive() must populate typed fact partition {partition:?}"
        );
    }

    let shape_use = FactSubstrate::derive_shape_and_use(&program);
    assert!(shape_use.has_fresh_partition_for(&program, FactPartition::Graph));
    assert!(shape_use.has_fresh_partition_for(&program, FactPartition::Shape));
    assert!(shape_use.has_fresh_partition_for(&program, FactPartition::Use));
    assert!(shape_use.has_fresh_partition_for(&program, FactPartition::Effects));
    assert!(shape_use.has_fresh_partition_for(&program, FactPartition::DataflowFrontier));
    assert!(!shape_use.has_fresh_partition_for(&program, FactPartition::Type));

    let use_only = FactSubstrate::derive_use_only(&program);
    assert!(use_only.has_fresh_partition_for(&program, FactPartition::Use));
    assert!(use_only.has_fresh_partition_for(&program, FactPartition::Effects));
    assert!(use_only.has_fresh_partition_for(&program, FactPartition::DataflowFrontier));
    assert!(!use_only.has_fresh_partition_for(&program, FactPartition::Graph));
    assert!(!use_only.has_fresh_partition_for(&program, FactPartition::Shape));
    assert!(!use_only.has_fresh_partition_for(&program, FactPartition::Type));
}

#[test]
fn typed_partition_invalidation_preserves_unrelated_fact_families() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let mut substrate = FactSubstrate::derive(&program);

    substrate.invalidate_partition(FactPartition::Shape);
    assert!(!substrate.has_partition(FactPartition::Shape));
    assert!(!substrate.has_partition(FactPartition::Graph));
    assert!(substrate.has_partition(FactPartition::Use));
    assert!(substrate.has_partition(FactPartition::Type));

    substrate.invalidate_partition(FactPartition::DataflowFrontier);
    assert!(!substrate.has_partition(FactPartition::Use));
    assert!(!substrate.has_partition(FactPartition::Effects));
    assert!(!substrate.has_partition(FactPartition::DataflowFrontier));
    assert!(substrate.has_partition(FactPartition::Type));
}

#[test]
fn derive_use_counts_handles_large_blocks_in_one_pass() {
    let block = Node::block(
        (0..4096)
            .map(|index| Node::let_bind(format!("sink_{index}"), Expr::var("x")))
            .collect(),
    );
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::u32(1)), block],
    );
    let substrate = FactSubstrate::derive(&program);
    assert_eq!(substrate.use_count_of(&Ident::from("x")), 4096);
}

#[test]
fn derive_cached_returns_equivalent_facts() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    );
    let direct = FactSubstrate::derive(&program);
    let cached = FactSubstrate::derive_cached(&program);
    let cached_again = FactSubstrate::derive_cached(&program);
    assert_eq!(direct.use_count_of(&Ident::from("x")), 1);
    assert_eq!(cached.use_count_of(&Ident::from("x")), 1);
    assert_eq!(cached_again.use_count_of(&Ident::from("x")), 1);
    let direct_use_facts = direct
        .use_facts()
        .expect("Fix: derive must populate use_facts");
    let cached_use_facts = cached
        .use_facts()
        .expect("Fix: derive_cached must populate use_facts");
    assert_eq!(
        direct_use_facts.buffer_writes,
        cached_use_facts.buffer_writes
    );
}

#[test]
fn derive_use_only_cached_returns_equivalent_facts() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(7)),
            Node::let_bind("b", Expr::add(Expr::var("a"), Expr::var("a"))),
            Node::store("out", Expr::u32(0), Expr::var("b")),
        ],
    );
    let direct = FactSubstrate::derive_use_only(&program);
    let cached = FactSubstrate::derive_use_only_cached(&program);
    let cached_again = FactSubstrate::derive_use_only_cached(&program);
    for (s, label) in [
        (&direct, "direct"),
        (&cached, "cached"),
        (&cached_again, "cached_again"),
    ] {
        assert_eq!(s.use_count_of(&Ident::from("a")), 2, "{label}");
        assert_eq!(s.use_count_of(&Ident::from("b")), 1, "{label}");
    }
}

#[test]
fn derive_shape_and_use_cached_keys_on_program_fingerprint() {
    let program_a = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let program_b = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(8)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let cached_a = FactSubstrate::derive_shape_and_use_cached(&program_a);
    let cached_b = FactSubstrate::derive_shape_and_use_cached(&program_b);
    let cached_a_again = FactSubstrate::derive_shape_and_use_cached(&program_a);
    assert_eq!(cached_a.fingerprint, program_a.fingerprint());
    assert_eq!(cached_b.fingerprint, program_b.fingerprint());
    assert_eq!(cached_a_again.fingerprint, program_a.fingerprint());
    assert_ne!(cached_a.fingerprint, cached_b.fingerprint);
}
