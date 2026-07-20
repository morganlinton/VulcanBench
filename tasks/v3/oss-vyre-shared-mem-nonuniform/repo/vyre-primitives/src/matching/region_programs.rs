use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable op id for survivor-flag generation over sorted region triples.
pub const DEDUP_REGIONS_FLAG_OP_ID: &str = "vyre-primitives::matching::region::dedup_regions_flag";
/// Stable op id for full cluster metadata over sorted region triples.
pub const DEDUP_REGIONS_CLUSTER_OP_ID: &str =
    "vyre-primitives::matching::region::dedup_regions_cluster";
/// Region-dedup lane packing for scanner match buffers.
pub const REGION_DEDUP_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for region-dedup match buffers.
#[must_use]
pub const fn region_dedup_dispatch_grid(count: u32) -> [u32; 3] {
    let blocks = count.div_ceil(REGION_DEDUP_WORKGROUP_SIZE[0]);
    if blocks == 0 {
        [1, 1, 1]
    } else {
        [blocks, 1, 1]
    }
}

/// GPU survivor-flag companion to `region::dedup_regions_inplace`.
///
/// Input contract: `pids`, `starts`, `ends` are three parallel
/// storage buffers, sorted by `(pid, start, end)`  -  the same order
/// the CPU reference produces after `sort_unstable`. Each lane scans
/// earlier same-pid spans and writes a `0`/`1` survivor flag into the
/// `survivors` buffer. The flag is `1` only when the slot starts a new
/// maximal overlap/touch cluster. Nested spans therefore merge into the
/// first cluster slot even when the immediately previous span is short.
///
/// Composition: pair this Program with
/// [`dedup_regions_cluster_program`] when compacted output must carry
/// the merged end offset as well as the survivor start slot. The flag
/// program stays available for consumers that only need cluster starts
/// or already compute merged ends through another pipeline stage.
///
/// Use [`region_dedup_dispatch_grid`] for explicit launches.
#[must_use]
pub fn dedup_regions_flag_program(
    pids: &str,
    starts: &str,
    ends: &str,
    survivors: &str,
    count: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(count)),
        dedup_regions_cluster_nodes(pids, starts, ends, survivors, None, count, t.clone()),
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(pids, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(starts, 1, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(ends, 2, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(survivors, 3, BufferAccess::WriteOnly, DataType::U32)
                .with_count(count),
        ],
        REGION_DEDUP_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(DEDUP_REGIONS_FLAG_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// GPU cluster-metadata companion to `region::dedup_regions_inplace`.
///
/// The program consumes sorted `(pid, start, end)` columns and writes:
///
/// - `survivors[i] = 1` for the first lane of each maximal same-pid
///   overlap/touch cluster, otherwise `0`.
/// - `merged_ends[i] = max(end)` for that cluster when `survivors[i]`
///   is `1`. Non-survivor lanes receive their own `end` value and are
///   ignored by stream-compaction.
///
/// After this program, compact `pids`, `starts`, and `merged_ends`
/// with the same survivor flags to obtain GPU-resident deduplicated
/// region triples matching the CPU reference.
#[must_use]
pub fn dedup_regions_cluster_program(
    pids: &str,
    starts: &str,
    ends: &str,
    survivors: &str,
    merged_ends: &str,
    count: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(count)),
        dedup_regions_cluster_nodes(
            pids,
            starts,
            ends,
            survivors,
            Some(merged_ends),
            count,
            t.clone(),
        ),
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(pids, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(starts, 1, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(ends, 2, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(survivors, 3, BufferAccess::WriteOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage(merged_ends, 4, BufferAccess::WriteOnly, DataType::U32)
                .with_count(count),
        ],
        REGION_DEDUP_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(DEDUP_REGIONS_CLUSTER_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

fn dedup_regions_cluster_nodes(
    pids: &str,
    starts: &str,
    ends: &str,
    survivors: &str,
    merged_ends: Option<&str>,
    count: u32,
    t: Expr,
) -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("pid_i", Expr::load(pids, t.clone())),
        Node::let_bind("start_i", Expr::load(starts, t.clone())),
        Node::let_bind("end_i", Expr::load(ends, t.clone())),
        Node::let_bind("has_prev_overlap", Expr::u32(0)),
        Node::loop_for(
            "j",
            Expr::u32(0),
            t.clone(),
            vec![
                Node::let_bind("pid_j", Expr::load(pids, Expr::var("j"))),
                Node::let_bind("end_j", Expr::load(ends, Expr::var("j"))),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("pid_j"), Expr::var("pid_i")),
                        Expr::ge(Expr::var("end_j"), Expr::var("start_i")),
                    ),
                    vec![Node::assign("has_prev_overlap", Expr::u32(1))],
                ),
            ],
        ),
        Node::let_bind(
            "survivor",
            Expr::select(
                Expr::eq(Expr::var("has_prev_overlap"), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            ),
        ),
        Node::store(survivors, t.clone(), Expr::var("survivor")),
    ];

    if let Some(merged_ends) = merged_ends {
        nodes.extend([
            Node::let_bind("merged_end", Expr::var("end_i")),
            Node::let_bind("cluster_active", Expr::var("survivor")),
            Node::loop_for(
                "k",
                Expr::add(t.clone(), Expr::u32(1)),
                Expr::u32(count),
                vec![
                    Node::let_bind("pid_k", Expr::load(pids, Expr::var("k"))),
                    Node::let_bind("start_k", Expr::load(starts, Expr::var("k"))),
                    Node::let_bind("end_k", Expr::load(ends, Expr::var("k"))),
                    Node::let_bind("same_pid", Expr::eq(Expr::var("pid_k"), Expr::var("pid_i"))),
                    Node::let_bind(
                        "touches_cluster",
                        Expr::le(Expr::var("start_k"), Expr::var("merged_end")),
                    ),
                    Node::let_bind(
                        "merge_k",
                        Expr::and(
                            Expr::eq(Expr::var("cluster_active"), Expr::u32(1)),
                            Expr::and(Expr::var("same_pid"), Expr::var("touches_cluster")),
                        ),
                    ),
                    Node::if_then(
                        Expr::var("merge_k"),
                        vec![Node::assign(
                            "merged_end",
                            Expr::max(Expr::var("merged_end"), Expr::var("end_k")),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("cluster_active"), Expr::u32(1)),
                            Expr::or(
                                Expr::ne(Expr::var("pid_k"), Expr::var("pid_i")),
                                Expr::and(
                                    Expr::var("same_pid"),
                                    Expr::gt(Expr::var("start_k"), Expr::var("merged_end")),
                                ),
                            ),
                        ),
                        vec![Node::assign("cluster_active", Expr::u32(0))],
                    ),
                ],
            ),
            Node::store(merged_ends, t, Expr::var("merged_end")),
        ]);
    }

    nodes
}

/// GPU stable rank sort of three parallel `(pid, start, end)` buffers
/// by composite lexicographic key  -  closes the host-side sort gap in
/// the dedup pipeline.
///
/// Pairs with [`dedup_regions_cluster_program`] and stream compaction:
///
/// ```text
/// region_sort_program(in_p, in_s, in_e, out_p, out_s, out_e, n)
///   -> dedup_regions_cluster_program(out_p, out_s, out_e, flags, merged, n)
///   -> prefix_scan(flags, offsets, n)
///   -> stream_compact(pids/starts/merged)
/// ```
///
/// Each invocation `i` computes its rank among the input by counting
/// how many input slots `j` carry a strictly-smaller composite key,
/// plus a stable tie-break (`j < i` for equal keys). The output
/// triples land at the rank position.
#[must_use]
pub fn region_sort_program(
    pids_in: &str,
    starts_in: &str,
    ends_in: &str,
    pids_out: &str,
    starts_out: &str,
    ends_out: &str,
    count: u32,
) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            "vyre-primitives::matching::region::sort_regions",
            pids_out,
            DataType::U32,
            format!("Fix: region_sort_program requires count > 0, got {count}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let pid_eq = Expr::eq(Expr::var("pid_j"), Expr::var("pid_i"));
    let start_eq = Expr::eq(Expr::var("start_j"), Expr::var("start_i"));
    let lower_key = Expr::or(
        Expr::lt(Expr::var("pid_j"), Expr::var("pid_i")),
        Expr::or(
            Expr::and(
                pid_eq.clone(),
                Expr::lt(Expr::var("start_j"), Expr::var("start_i")),
            ),
            Expr::and(
                pid_eq.clone(),
                Expr::and(
                    start_eq.clone(),
                    Expr::lt(Expr::var("end_j"), Expr::var("end_i")),
                ),
            ),
        ),
    );
    let stable_tie = Expr::and(
        pid_eq,
        Expr::and(
            start_eq,
            Expr::and(
                Expr::eq(Expr::var("end_j"), Expr::var("end_i")),
                Expr::lt(Expr::var("j"), Expr::var("i")),
            ),
        ),
    );

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(count)),
        vec![
            Node::let_bind("i", t.clone()),
            Node::let_bind("pid_i", Expr::load(pids_in, Expr::var("i"))),
            Node::let_bind("start_i", Expr::load(starts_in, Expr::var("i"))),
            Node::let_bind("end_i", Expr::load(ends_in, Expr::var("i"))),
            Node::let_bind("rank", Expr::u32(0)),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(count),
                vec![
                    Node::let_bind("pid_j", Expr::load(pids_in, Expr::var("j"))),
                    Node::let_bind("start_j", Expr::load(starts_in, Expr::var("j"))),
                    Node::let_bind("end_j", Expr::load(ends_in, Expr::var("j"))),
                    Node::if_then(
                        Expr::or(lower_key.clone(), stable_tie.clone()),
                        vec![Node::assign(
                            "rank",
                            Expr::add(Expr::var("rank"), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
            Node::store(pids_out, Expr::var("rank"), Expr::var("pid_i")),
            Node::store(starts_out, Expr::var("rank"), Expr::var("start_i")),
            Node::store(ends_out, Expr::var("rank"), Expr::var("end_i")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(pids_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage(starts_in, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage(ends_in, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage(pids_out, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
            BufferDecl::storage(starts_out, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
            BufferDecl::storage(ends_out, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
        ],
        REGION_DEDUP_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from("vyre-primitives::matching::region::region_sort"),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}
