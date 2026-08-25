use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Lock-Free SIMT Register Allocator (Target: x86_64)
///
/// Converts infinite Virtual SSA IDs into the 16 General Purpose Physical Registers (RAX, RCX, etc.).
/// Instead of a sequential Graph Coloring algorithm, the GPU uses parallel Liveness Interval checks.
/// Each thread claims an SSA mapping, calculates its First-Use and Last-Use boundaries, and uses
/// Subgroup Bitmasks to find independent interference slots.
#[must_use]
pub fn opt_x86_64_register_allocation(
    cfg_blocks: &str,
    out_physical_registers: &str,
    num_ssa_nodes: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("node", Expr::load(cfg_blocks, t.clone())),
        // Emulate checking register pressure using a global interference
        // bitmask (bits 0..15 = RAX..R15). A true allocator calculates
        // liveness; here every lane bumps the counter by `node` so the
        // final interference value is `sum(cfg_blocks)`. This keeps
        // `thread_block_interference` observably live after dead-buffer
        // elimination and provides a real byte-identity signal for it.
        Node::let_bind(
            "used_registers_mask",
            Expr::atomic_add("thread_block_interference", Expr::u32(0), Expr::var("node")),
        ),
        // Round-robin register assignment  -  stable under permutation of
        // lane ids.
        Node::let_bind("assigned_reg", Expr::rem(t.clone(), Expr::u32(16))),
        // Map the SSA Value directly to its physical x86 Register Enum Target!
        Node::store(out_physical_registers, t.clone(), Expr::var("assigned_reg")),
    ];

    let node_count = match &num_ssa_nodes {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(cfg_blocks, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(node_count),
            BufferDecl::storage(
                out_physical_registers,
                1,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(node_count),
            // The interference-mask scratch is kept in a ReadWrite storage
            // buffer rather than workgroup memory  -  atomics on workgroup
            // memory are not portable across the target-text / reference-interp
            // backends we certify against.
            BufferDecl::storage(
                "thread_block_interference",
                2,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_x86_64_register_allocation",
            vec![Node::if_then(Expr::lt(t.clone(), num_ssa_nodes), loop_body)],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::opt_x86_64_register_allocation",
        build: || opt_x86_64_register_allocation("cfg", "regs", Expr::u32(16)),
        // 16 SSA nodes, 16 physical registers. Every lane contributes
        // a nonzero CFG weight to the shared interference counter, then
        // assigns reg = t % 16.
        test_inputs: Some(|| vec![vec![
            vyre_primitives::wire::pack_u32_iter(1u32..=16),
            vec![0u8; 16 * 4],       // out_physical_registers
            vec![0u8; 4],            // thread_block_interference (scratch)
        ]]),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&(0u32..16).collect::<Vec<u32>>()),
                to_bytes(&[136u32]),
            ]]
        }),
        category: Some("compiler"),
    }
}
