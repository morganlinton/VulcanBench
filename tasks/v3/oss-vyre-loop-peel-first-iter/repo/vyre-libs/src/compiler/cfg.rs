use crate::region::{wrap_anonymous, wrap_child};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

/// SIMT Control Flow Graph (CFG) & Goto Resolver.
///
/// Linux-kernel C uses `goto` pervasively for error unwind. This pass
/// walks the flat SSA node array, registers every `label:` site into
/// a lock-free open-addressing hash table (`goto_labels_*`), and
/// resolves every `goto target;` site against that table to a CFG
/// edge written into `out_cfg_blocks`.
///
/// The hash insert / lookup use linear probing on a 4096-slot table
/// with `AtomicCompareExchange` for first-writer-wins registration
/// and a bounded linear scan for lookup. No external primitive
/// dependency  -  the kernel is self-contained.
///
/// Opcode sentinels (ASCII tag bytes): `0x4C41424C` = "LABL" (label
/// definition), `0x474F544F` = "GOTO" (goto site).
#[must_use]
pub fn c11_build_cfg_and_gotos(
    ssa_nodes: &str,
    out_cfg_blocks: &str,
    goto_labels: &str,
    num_ssa: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    const TABLE_CAP: u32 = 4096;
    const EMPTY_SLOT: u32 = 0xFFFF_FFFF;

    // Shared-across-branches: a bounded linear-probe body that
    // scans up to `TABLE_CAP` slots starting at `start_slot`.
    let hash_slot = |key: Expr| -> Expr {
        // Fibonacci hash, capped to TABLE_CAP.
        Expr::bitand(
            Expr::mul(key, Expr::u32(2_654_435_769)),
            Expr::u32(TABLE_CAP - 1),
        )
    };

    let label_phase = vec![
        Node::let_bind("opcode", Expr::load(ssa_nodes, t.clone())),
        // 1. Label definition: register `label_hash -> ssa_index` in the
        //    open-addressing hash table. Linear probe with atomic CAS
        //    keyed on EMPTY_SLOT so only the first writer wins.
        Node::if_then(
            Expr::eq(Expr::var("opcode"), Expr::u32(0x4C41424C)),
            vec![
                Node::let_bind(
                    "label_hash",
                    Expr::load(ssa_nodes, Expr::add(t.clone(), Expr::u32(1))),
                ),
                Node::let_bind("slot", hash_slot(Expr::var("label_hash"))),
                Node::let_bind("inserted", Expr::u32(0)),
                Node::loop_for(
                    "probe",
                    Expr::u32(0),
                    Expr::u32(TABLE_CAP),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("inserted"), Expr::u32(0)),
                        vec![
                            Node::let_bind(
                                "prev",
                                Expr::atomic_compare_exchange(
                                    "goto_labels_keys",
                                    Expr::var("slot"),
                                    Expr::u32(EMPTY_SLOT),
                                    Expr::var("label_hash"),
                                ),
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("prev"), Expr::u32(EMPTY_SLOT)),
                                vec![
                                    Node::store("goto_labels_vals", Expr::var("slot"), t.clone()),
                                    Node::assign("inserted", Expr::u32(1)),
                                ],
                            ),
                            Node::assign(
                                "slot",
                                Expr::bitand(
                                    Expr::add(Expr::var("slot"), Expr::u32(1)),
                                    Expr::u32(TABLE_CAP - 1),
                                ),
                            ),
                        ],
                    )],
                ),
            ],
        ),
    ];
    let goto_phase = vec![
        Node::let_bind("opcode", Expr::load(ssa_nodes, t.clone())),
        // 2. Goto site: look up `target_hash` in the same table, write
        //    the resolved SSA index (or EMPTY_SLOT on miss) into
        //    `out_cfg_blocks[t]`.
        //
        // Lookup uses first-match semantics: the first slot whose key
        // equals `target_hash` wins.  Two invariants that allow early exit:
        //   (a) `found == 1` after the first match — skip all further slots.
        //   (b) `k == EMPTY_SLOT` means no further entries exist in a
        //       linear-probe table, so the target is absent.
        // Both guards are implemented under `Expr::eq(found, 0)` because
        // the IR `loop_for` has no break node.
        Node::if_then(
            Expr::eq(Expr::var("opcode"), Expr::u32(0x474F544F)),
            vec![
                Node::let_bind(
                    "target_hash",
                    Expr::load(ssa_nodes, Expr::add(t.clone(), Expr::u32(1))),
                ),
                Node::let_bind("lk_slot", hash_slot(Expr::var("target_hash"))),
                Node::let_bind("resolved", Expr::u32(EMPTY_SLOT)),
                Node::let_bind("found", Expr::u32(0)),
                Node::loop_for(
                    "probe",
                    Expr::u32(0),
                    Expr::u32(TABLE_CAP),
                    vec![
                        // Only probe further when we have not yet found a match.
                        Node::if_then(
                            Expr::eq(Expr::var("found"), Expr::u32(0)),
                            vec![
                                Node::let_bind(
                                    "k",
                                    Expr::load("goto_labels_keys", Expr::var("lk_slot")),
                                ),
                                // Stop on the first key match — first-writer-wins.
                                Node::if_then(
                                    Expr::eq(Expr::var("k"), Expr::var("target_hash")),
                                    vec![
                                        Node::assign(
                                            "resolved",
                                            Expr::load(
                                                "goto_labels_vals",
                                                Expr::var("lk_slot"),
                                            ),
                                        ),
                                        Node::assign("found", Expr::u32(1)),
                                    ],
                                ),
                                // Stop also when we hit an empty slot: no further
                                // entries can follow in a linear-probe table.
                                Node::if_then(
                                    Expr::eq(Expr::var("k"), Expr::u32(EMPTY_SLOT)),
                                    vec![Node::assign("found", Expr::u32(1))],
                                ),
                                Node::assign(
                                    "lk_slot",
                                    Expr::bitand(
                                        Expr::add(Expr::var("lk_slot"), Expr::u32(1)),
                                        Expr::u32(TABLE_CAP - 1),
                                    ),
                                ),
                            ],
                        ),
                    ],
                ),
                Node::store(out_cfg_blocks, t.clone(), Expr::var("resolved")),
            ],
        ),
    ];

    let n_ssa = match &num_ssa {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(ssa_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_ssa),
            BufferDecl::storage(out_cfg_blocks, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_ssa),
            BufferDecl::storage(goto_labels, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_ssa),
            BufferDecl::storage(
                "goto_labels_keys",
                3,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(TABLE_CAP)
            .with_output_byte_range(0..0),
            BufferDecl::storage(
                "goto_labels_vals",
                4,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(TABLE_CAP)
            .with_output_byte_range(0..0),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_build_cfg_and_gotos",
            vec![wrap_child(
                vyre_primitives::graph::csr_forward_traverse::OP_ID,
                GeneratorRef {
                    name: "vyre-libs::parsing::c11_build_cfg_and_gotos".to_string(),
                },
                vec![
                    Node::if_then(Expr::lt(t.clone(), num_ssa.clone()), label_phase),
                    Node::barrier(),
                    Node::if_then(Expr::lt(t.clone(), num_ssa), goto_phase),
                ],
            )],
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    /// Regression: two LABL entries with the same label_hash (collision in the
    /// linear-probe table) must resolve a GOTO to the FIRST inserted label, not
    /// the last one.  Before the fix the lookup loop had no early-exit, so the
    /// second colliding slot would overwrite `resolved` and the wrong SSA index
    /// was emitted.
    ///
    /// Layout (7 invocations, each thread t reads ssa_nodes[t] as opcode):
    ///   t=0  NOP    (0)
    ///   t=1  LABL   opcode, hash at t=2 -> SSA index of this label is 1
    ///   t=2  7      (label_hash for t=1's LABL -- t=2 itself is not an opcode thread)
    ///   t=3  LABL   opcode, hash at t=4 -> same hash 7, second writer -> linear-probes to slot+1
    ///   t=4  7
    ///   t=5  GOTO   opcode, target_hash at t=6
    ///   t=6  7      (resolves via first-match -> t=1, i.e. SSA index 1)
    #[test]
    fn goto_collision_resolves_to_first_label_not_last() {
        // Build with num_ssa=7 so all 7 threads are in-bounds.
        let program = c11_build_cfg_and_gotos("ssa", "cfg", "labels", Expr::u32(7));
        let mut ssa = vec![0u8; 7 * 4];
        for (i, v) in [0u32, 0x4C41424C, 7, 0x4C41424C, 7, 0x474F544F, 7]
            .iter()
            .enumerate()
        {
            ssa[i * 4..(i + 1) * 4].copy_from_slice(&v.to_le_bytes());
        }
        // goto_labels_keys initialised to EMPTY_SLOT (0xFFFFFFFF).
        let keys_init = vec![0xFFu8; 4 * 4096];
        let vals_init = vec![0u8; 4 * 4096];
        let inputs = vec![
            Value::from(ssa),
            Value::from(vec![0u8; 7 * 4]), // out_cfg_blocks
            Value::from(vec![0u8; 7 * 4]), // goto_labels (labels alias buffer)
            Value::from(keys_init),
            Value::from(vals_init),
        ];
        let outputs =
            vyre_reference::reference_eval(&program, &inputs).expect("cfg eval must succeed");
        let cfg = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        // t=5 is the GOTO thread.  With first-match semantics it must resolve to
        // the SSA index of the FIRST LABL with hash 7, which is t=1.
        assert_eq!(
            cfg[5], 1,
            "goto must resolve to first label (SSA index 1), not last colliding label (SSA index 3); \
             got cfg[5]={}",
            cfg[5]
        );
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_build_cfg_and_gotos",
        build: || c11_build_cfg_and_gotos("ssa", "cfg", "labels", Expr::u32(5)),
        test_inputs: Some(|| {
            let mut ssa = Vec::with_capacity(5 * 4);
            for value in [0u32, 0x4C41424C, 7, 0x474F544F, 7] {
                ssa.extend_from_slice(&value.to_le_bytes());
            }
            vec![vec![
                ssa,
                vec![0u8; 5 * 4],
                vec![0u8; 5 * 4],
                vec![0xFFu8; 4 * 4096],
                vec![0u8; 4 * 4096],
            ]]
        }),
        expected_output: Some(|| {
            let mut cfg = Vec::with_capacity(5 * 4);
            for value in [0u32, 0, 0, 1, 0] {
                cfg.extend_from_slice(&value.to_le_bytes());
            }

            let labels = vec![0u8; 5 * 4];

            vec![vec![cfg, labels, Vec::new(), Vec::new()]]
        }),
        category: Some("compiler"),
    }
}
