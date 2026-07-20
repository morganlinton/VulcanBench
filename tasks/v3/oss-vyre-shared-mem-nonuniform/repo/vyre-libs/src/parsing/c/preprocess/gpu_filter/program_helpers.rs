use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub(super) const GPU_FILTER_WORKGROUP: [u32; 3] = [256, 1, 1];

pub(super) fn source_byte_load(buffer: &'static str, addr: Expr) -> Expr {
    let len = Expr::buf_len(buffer);
    let safe_addr = Expr::select(
        Expr::lt(addr.clone(), len.clone()),
        addr,
        Expr::saturating_sub(len, Expr::u32(1)),
    );
    Expr::bitand(
        Expr::cast(DataType::U32, Expr::load(buffer, safe_addr)),
        Expr::u32(0xFF),
    )
}

pub(super) fn source_byte_load_or_zero(
    buffer: &'static str,
    addr: Expr,
    real_len_buffer: &'static str,
) -> Expr {
    Expr::select(
        Expr::lt(addr.clone(), Expr::load(real_len_buffer, Expr::u32(0))),
        source_byte_load(buffer, addr),
        Expr::u32(0),
    )
}

pub(super) fn byte_eq(byte: Expr, expected: u8) -> Expr {
    Expr::eq(byte, Expr::u32(expected as u32))
}

pub(super) fn store_comment_mask(i: Expr, comment_mask: Expr) -> Node {
    Node::store("comment_mask_out", i, comment_mask)
}

pub(super) fn store_final_keep_from_comment_mask(i: Expr, comment_mask: Expr) -> Node {
    Node::store(
        "final_keep",
        i,
        Expr::select(
            Expr::ne(comment_mask, Expr::u32(1)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    )
}

pub(super) fn clear_comment_mask_and_final_keep(i: Expr) -> Vec<Node> {
    vec![
        store_comment_mask(i.clone(), Expr::u32(0)),
        Node::store("final_keep", i, Expr::u32(0)),
    ]
}

pub(super) fn source_bytes_input_buffer(name: &str, binding: u32, n: u32) -> BufferDecl {
    let _ = n;
    BufferDecl::storage(name, binding, BufferAccess::ReadOnly, DataType::U8).with_count(0)
}

pub(super) fn u32_read_buffer(name: &str, binding: u32, n: u32) -> BufferDecl {
    u32_storage_buffer(name, binding, BufferAccess::ReadOnly, n.max(1))
}

pub(super) fn u32_rw_buffer(name: &str, binding: u32, n: u32) -> BufferDecl {
    u32_storage_buffer(name, binding, BufferAccess::ReadWrite, n.max(1))
}

pub(super) fn singleton_u32_read_buffer(name: &str, binding: u32) -> BufferDecl {
    u32_storage_buffer(name, binding, BufferAccess::ReadOnly, 1)
}

fn u32_storage_buffer(name: &str, binding: u32, access: BufferAccess, count: u32) -> BufferDecl {
    BufferDecl::storage(name, binding, access, DataType::U32).with_count(count)
}

pub(super) fn wrap_gpu_filter_program(
    entry_op_id: &'static str,
    buffers: Vec<BufferDecl>,
    body: Vec<Node>,
) -> Program {
    Program::wrapped(buffers, GPU_FILTER_WORKGROUP, body).with_entry_op_id(entry_op_id)
}

/// Element-wise keep-mask merge over line-splice and comment metadata.
///
/// Input buffer names match the producing kernels' output names
/// (`kept_mask_out` from `line_splice_classify` and
/// `comment_mask_out` from `gpu_comment_strip_mask`; `0=original`,
/// `1=drop`, `2=replacement space`) so that
/// `fuse_programs` can wire the three stages into a single dispatch
/// without renaming. Output is `final_keep`.
///
/// `n` is the bucketed kernel extent (used for buffer counts and the
/// per-thread index bound). `final_keep_n_real[0]` is the runtime real
/// byte count: positions in [n_real, n) are forced to `final_keep=0`
/// so that bucket-padding bytes (which the upstream `line_splice` and
/// `gpu_comment_strip_mask` kernels classify as kept-and-not-comment
/// because they're zeros) don't scatter into the compacted output.
/// Reading `n_real` from a single-element buffer at runtime keeps the
/// program text uniform across files of different real lengths so the
/// dispatcher's pipeline cache hits across the corpus.
pub(super) fn combine_keep_mask_program(n: u32) -> Program {
    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("n_real", Expr::load("final_keep_n_real", Expr::u32(0))),
                Node::let_bind("sk", Expr::load("kept_mask_out", i.clone())),
                Node::let_bind("ck", Expr::load("comment_mask_out", i.clone())),
                Node::let_bind(
                    "out",
                    Expr::select(
                        Expr::and(
                            Expr::lt(i.clone(), Expr::var("n_real")),
                            Expr::and(
                                Expr::eq(Expr::var("sk"), Expr::u32(1)),
                                Expr::ne(Expr::var("ck"), Expr::u32(1)),
                            ),
                        ),
                        Expr::u32(1),
                        Expr::u32(0),
                    ),
                ),
                Node::store("final_keep", i.clone(), Expr::var("out")),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("kept_mask_out", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("comment_mask_out", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("final_keep", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage(
                "final_keep_n_real",
                3,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(1),
        ],
        [256, 1, 1],
        body,
    )
    .with_entry_op_id("vyre-libs::parsing::c::preprocess::gpu_pipeline::combine_keep_mask")
}

/// Per-byte compact: if `mask[i] == 1`, write `bytes_in[i]` or the
/// C-required comment replacement space to `compacted_out[offsets[i] - 1]`,
/// where `offsets` is an inclusive prefix sum over the keep-mask. Last lane
/// writes the survivor count.
///
/// The source is a raw `DataType::U8` byte buffer. `compacted_out` remains
/// packed U32 words because downstream C lexer stages still consume packed
/// words; each source byte is scattered into its destination word via
/// `atomic_or`. Output buffer must be zero-initialized by the host so the OR
/// accumulates correctly.
pub(super) fn byte_compact_program(n: u32) -> Program {
    // One thread per *output word* (not per input byte). Each thread
    // handles up to 4 input bytes at indices [4w, 4w+1, 4w+2, 4w+3],
    // checks each one's mask, and atomic-or's its byte into the
    // packed output word at the prefix-scan offset. The thread that
    // covers the last input byte (`w == (n-1)/4`) computes the live
    // count from its in-window slot.
    //
    // This shape was chosen over the simpler "one thread per input
    // byte" because the dispatcher infers the launch grid from the
    // primary output buffer's `count`. `compacted_out` is naturally
    // sized at `ceil(n/4)` u32 words, so a per-input-byte kernel
    // under-dispatched whenever n > workgroup_size (256 threads ran
    // but the loop indexed 0..n; bytes past 256 were silently
    // skipped, the last-thread `live_count_out` store never fired,
    // and the host saw `live=0`). A per-word kernel makes the kernel's
    // logical extent match the inferred grid exactly with no over-
    // allocation, no DispatchConfig override needed, and no false
    // dependency between primary-output word count and input length.
    let words = n.div_ceil(4).max(1);
    let w = Expr::var("w");
    fn process_byte_arm(k: u32, n: u32) -> Vec<Node> {
        let i = Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k));
        vec![Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind(format!("m_{k}"), Expr::load("mask", i.clone())),
                Node::let_bind(format!("off_{k}"), Expr::load("offsets", i.clone())),
                Node::if_then(
                    Expr::eq(Expr::var(format!("m_{k}")), Expr::u32(1)),
                    vec![
                        Node::let_bind(format!("cm_{k}"), Expr::load("comment_mask", i.clone())),
                        Node::let_bind(
                            format!("in_byte_{k}"),
                            Expr::select(
                                Expr::eq(Expr::var(format!("cm_{k}")), Expr::u32(2)),
                                Expr::u32(b' ' as u32),
                                source_byte_load("bytes_in", i.clone()),
                            ),
                        ),
                        Node::let_bind(
                            format!("out_pos_{k}"),
                            Expr::saturating_sub(Expr::var(format!("off_{k}")), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            format!("out_word_idx_{k}"),
                            Expr::div(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
                        ),
                        Node::let_bind(
                            format!("out_shift_{k}"),
                            Expr::mul(
                                Expr::rem(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
                                Expr::u32(8),
                            ),
                        ),
                        Node::let_bind(
                            format!("shifted_byte_{k}"),
                            Expr::shl(
                                Expr::var(format!("in_byte_{k}")),
                                Expr::var(format!("out_shift_{k}")),
                            ),
                        ),
                        Node::let_bind(
                            format!("_prev_{k}"),
                            Expr::atomic_or(
                                "compacted_out",
                                Expr::var(format!("out_word_idx_{k}")),
                                Expr::var(format!("shifted_byte_{k}")),
                            ),
                        ),
                    ],
                ),
                // The thread covering the last input byte commits the
                // live count: inclusive_offsets[n-1] is the total kept.
                Node::if_then(
                    Expr::eq(i.clone(), Expr::u32(n - 1)),
                    vec![Node::store(
                        "live_count_out",
                        Expr::u32(0),
                        Expr::var(format!("off_{k}")),
                    )],
                ),
            ],
        )]
    }
    let body = vec![
        Node::let_bind("w", Expr::InvocationId { axis: 0 }),
        Node::if_then(Expr::lt(w.clone(), Expr::u32(words)), {
            let mut arms = Vec::new();
            for k in 0..4u32 {
                arms.extend(process_byte_arm(k, n));
            }
            arms
        }),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("bytes_in", 0, BufferAccess::ReadOnly, DataType::U8).with_count(0),
            BufferDecl::storage("mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("comment_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("offsets", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("compacted_out", 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage("live_count_out", 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        body,
    )
    .with_entry_op_id("vyre-libs::parsing::c::preprocess::gpu_pipeline::byte_compact")
}

// GPU-roundtrip tests live in `tests/gpu_pipeline_filter_roundtrip.rs`
// because they drive the real dispatch backend.
