fn arb_node() -> BoxedStrategy<Node> {
    arb_node_with_depth(3)
}

fn arb_program() -> BoxedStrategy<Program> {
    (
        arb_buffer_datatype(),
        arb_buffer_datatype(),
        prop_vec(arb_node(), 0..=6),
        prop_oneof![9 => Just(false), 1 => Just(true)],
    )
        .prop_map(|(extra_a, extra_b, entry, non_composable)| {
            Program::wrapped(
                vec![
                    BufferDecl::output("out", 0, DataType::U32)
                        .with_count(8)
                        .with_output_byte_range(0..16),
                    BufferDecl::read("input", 1, DataType::U32).with_count(8),
                    BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
                    BufferDecl::read("bytes_in", 3, DataType::Bytes).with_count(16),
                    BufferDecl::read_write("bytes_out", 4, DataType::Bytes).with_count(16),
                    BufferDecl::read("counts", 5, DataType::U32).with_count(8),
                    BufferDecl::workgroup("scratch", 4, DataType::U32),
                    BufferDecl::read("extra_a", 6, extra_a).with_count(1),
                    BufferDecl::read("extra_b", 7, extra_b).with_count(1),
                ],
                [1, 1, 1],
                entry,
            )
            .with_non_composable_with_self(non_composable)
        })
        .boxed()
}

// ─── manual recompute functions (must mirror compute_stats exactly) ───

#[inline]
fn mark_datatype_bits(ty: &DataType, bits: &mut u32) {
    match ty {
        DataType::F16 => *bits |= CAP_F16,
        DataType::BF16 => *bits |= CAP_BF16,
        DataType::F64 => *bits |= CAP_F64,
        DataType::Tensor | DataType::TensorShaped { .. } => *bits |= CAP_TENSOR_OPS,
        _ => {}
    }
}

fn is_subgroup_intrinsic_id(op_id: &str) -> bool {
    const MARKERS: &[&str] = &[
        "subgroup_",
        "::subgroup::",
        "::subgroup",
        "wave_",
        "::wave::",
        "warp_",
        "::warp::",
    ];
    MARKERS.iter().any(|marker| op_id.contains(marker))
}

#[allow(clippy::only_used_in_recursion)]
fn manual_walk_expr(
    expr: &Expr,
    nodes: &mut usize,
    regions: &mut u32,
    calls: &mut u32,
    opaque: &mut u32,
    bits: &mut u32,
) {
    match expr {
        Expr::SubgroupReduce { value, .. } => {
            *bits |= CAP_SUBGROUP_OPS;
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
        }
        Expr::SubgroupBallot { cond } => {
            *bits |= CAP_SUBGROUP_OPS;
            manual_walk_expr(cond, nodes, regions, calls, opaque, bits);
        }
        Expr::SubgroupShuffle { value, lane } => {
            *bits |= CAP_SUBGROUP_OPS;
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
            manual_walk_expr(lane, nodes, regions, calls, opaque, bits);
        }
        Expr::BinOp { left, right, .. } => {
            manual_walk_expr(left, nodes, regions, calls, opaque, bits);
            manual_walk_expr(right, nodes, regions, calls, opaque, bits);
        }
        Expr::UnOp { operand, .. } => {
            manual_walk_expr(operand, nodes, regions, calls, opaque, bits)
        }
        Expr::Fma { a, b, c } => {
            manual_walk_expr(a, nodes, regions, calls, opaque, bits);
            manual_walk_expr(b, nodes, regions, calls, opaque, bits);
            manual_walk_expr(c, nodes, regions, calls, opaque, bits);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            manual_walk_expr(cond, nodes, regions, calls, opaque, bits);
            manual_walk_expr(true_val, nodes, regions, calls, opaque, bits);
            manual_walk_expr(false_val, nodes, regions, calls, opaque, bits);
        }
        Expr::Cast { target, value } => {
            mark_datatype_bits(target, bits);
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
        }
        Expr::Load { index, .. } => manual_walk_expr(index, nodes, regions, calls, opaque, bits),
        Expr::Call { op_id, args } => {
            if is_subgroup_intrinsic_id(op_id.as_str()) {
                *bits |= CAP_SUBGROUP_OPS;
            }
            *calls = calls.saturating_add(1);
            for arg in args.iter() {
                manual_walk_expr(arg, nodes, regions, calls, opaque, bits);
            }
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            manual_walk_expr(index, nodes, regions, calls, opaque, bits);
            if let Some(expected) = expected.as_deref() {
                manual_walk_expr(expected, nodes, regions, calls, opaque, bits);
            }
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
        }
        Expr::Opaque(_) => {
            *opaque = opaque.saturating_add(1);
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => {}
        _ => {}
    }
}

fn manual_walk_node(
    node: &Node,
    nodes: &mut usize,
    regions: &mut u32,
    calls: &mut u32,
    opaque: &mut u32,
    bits: &mut u32,
) {
    *nodes = nodes.saturating_add(1);
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
        }
        Node::Store { index, value, .. } => {
            manual_walk_expr(index, nodes, regions, calls, opaque, bits);
            manual_walk_expr(value, nodes, regions, calls, opaque, bits);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            manual_walk_expr(cond, nodes, regions, calls, opaque, bits);
            for child in then.iter().chain(otherwise.iter()) {
                manual_walk_node(child, nodes, regions, calls, opaque, bits);
            }
        }
        Node::Loop { from, to, body, .. } => {
            manual_walk_expr(from, nodes, regions, calls, opaque, bits);
            manual_walk_expr(to, nodes, regions, calls, opaque, bits);
            for child in body.iter() {
                manual_walk_node(child, nodes, regions, calls, opaque, bits);
            }
        }
        Node::Block(children) => {
            for child in children.iter() {
                manual_walk_node(child, nodes, regions, calls, opaque, bits);
            }
        }
        Node::Region { body, .. } => {
            *regions = regions.saturating_add(1);
            for child in body.iter() {
                manual_walk_node(child, nodes, regions, calls, opaque, bits);
            }
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            *bits |= CAP_ASYNC_DISPATCH;
            manual_walk_expr(offset, nodes, regions, calls, opaque, bits);
            manual_walk_expr(size, nodes, regions, calls, opaque, bits);
        }
        Node::AsyncWait { .. } => {
            *bits |= CAP_ASYNC_DISPATCH;
        }
        Node::IndirectDispatch { .. } => {
            *bits |= CAP_INDIRECT_DISPATCH;
        }
        Node::Trap { address, .. } => {
            *bits |= CAP_TRAP;
            manual_walk_expr(address, nodes, regions, calls, opaque, bits);
        }
        Node::Opaque(_) => {
            *opaque = opaque.saturating_add(1);
        }
        Node::Return | Node::Barrier { .. } | Node::Resume { .. } => {}
        _ => {}
    }
}

fn manual_compute_stats(program: &Program) -> ProgramStats {
    let mut node_count = 0usize;
    let mut region_count = 0u32;
    let mut call_count = 0u32;
    let mut opaque_count = 0u32;
    let mut capability_bits = 0u32;
    let mut static_storage_bytes = 0u64;

    for decl in program.buffers().iter() {
        let count = decl.count();
        if count != 0 {
            if let Some(elem) = decl.element().size_bytes() {
                static_storage_bytes =
                    static_storage_bytes.saturating_add(u64::from(count) * elem as u64);
            }
        }
        mark_datatype_bits(&decl.element(), &mut capability_bits);
    }

    for node in program.entry().iter() {
        manual_walk_node(
            node,
            &mut node_count,
            &mut region_count,
            &mut call_count,
            &mut opaque_count,
            &mut capability_bits,
        );
    }

    let top_level_regions = program
        .entry()
        .iter()
        .filter(|n| matches!(n, Node::Region { .. }))
        .count() as u32;

    ProgramStats {
        node_count,
        region_count,
        call_count,
        opaque_count,
        top_level_regions,
        static_storage_bytes,
        capability_bits,
        ..ProgramStats::default()
    }
}

// ─── proptest ───

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        .. ProptestConfig::default()
    })]

    #[test]
    fn program_stats_cache_invariants(program in arb_program()) {
        // 1. Identity: repeated calls return the same cached reference.
        let stats_a = program.stats();
        let stats_b = program.stats();
        prop_assert!(
            std::ptr::eq(stats_a, stats_b),
            "program.stats() must return the same cached reference on repeated calls"
        );

        // 2–4. Manual recompute must match every field.
        let manual = manual_compute_stats(&program);
        let cached = program.stats();

        prop_assert_eq!(
            cached.node_count, manual.node_count,
            "node_count mismatch: cached={}, manual={}", cached.node_count, manual.node_count
        );
        prop_assert_eq!(
            cached.region_count, manual.region_count,
            "region_count mismatch: cached={}, manual={}", cached.region_count, manual.region_count
        );
        prop_assert_eq!(
            cached.call_count, manual.call_count,
            "call_count mismatch: cached={}, manual={}", cached.call_count, manual.call_count
        );
        prop_assert_eq!(
            cached.opaque_count, manual.opaque_count,
            "opaque_count mismatch: cached={}, manual={}", cached.opaque_count, manual.opaque_count
        );
        prop_assert_eq!(
            cached.top_level_regions, manual.top_level_regions,
            "top_level_regions mismatch: cached={}, manual={}", cached.top_level_regions, manual.top_level_regions
        );
        prop_assert_eq!(
            cached.static_storage_bytes, manual.static_storage_bytes,
            "static_storage_bytes mismatch: cached={}, manual={}", cached.static_storage_bytes, manual.static_storage_bytes
        );
        prop_assert_eq!(
            cached.capability_bits, manual.capability_bits,
            "capability_bits mismatch: cached={:08b}, manual={:08b}", cached.capability_bits, manual.capability_bits
        );
    }
}
