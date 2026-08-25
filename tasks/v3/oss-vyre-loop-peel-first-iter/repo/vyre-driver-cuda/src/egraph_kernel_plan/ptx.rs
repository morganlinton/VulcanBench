//! CUDA PTX generation for resident e-graph kernels.

use super::{
    CudaEGraphCanonicalRewriteKernelPtx, CudaEGraphKernelPlanError,
    CudaEGraphSignatureRefreshKernelPtx, CudaEGraphStructuralEquivalenceKernelPtx,
    CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_ENTRY, CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS, CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS,
    CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_ENTRY, CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY,
    CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT,
};

/// Emit PTX for structural-equivalence row-pair comparison over packed e-graph signature buckets.
pub fn cuda_egraph_structural_equivalence_kernel_ptx(
    target_sm: u32,
) -> Result<CudaEGraphStructuralEquivalenceKernelPtx, CudaEGraphKernelPlanError> {
    let ptx_version = cuda_egraph_ptx_version(target_sm)?;
    let source = format!(
        r#".version {ptx_version}
.target sm_{target_sm}
.address_size 64

.visible .entry {entry}(
    .param .u64 row_eclass_ids_ptr,
    .param .u64 row_language_op_ids_ptr,
    .param .u64 row_children_offsets_ptr,
    .param .u64 row_children_lens_ptr,
    .param .u64 row_signatures_ptr,
    .param .u64 children_ptr,
    .param .u64 bucket_words_ptr,
    .param .u64 bucket_rows_ptr,
    .param .u64 output_pairs_ptr,
    .param .u64 output_count_ptr,
    .param .u32 bucket_index,
    .param .u64 first_pair,
    .param .u64 pair_count
)
{{
    .reg .pred %p<12>;
    .reg .b32 %r<40>;
    .reg .b64 %rd<80>;

    ld.param.u64 %rd1, [row_eclass_ids_ptr];
    ld.param.u64 %rd2, [row_language_op_ids_ptr];
    ld.param.u64 %rd3, [row_children_offsets_ptr];
    ld.param.u64 %rd4, [row_children_lens_ptr];
    ld.param.u64 %rd5, [row_signatures_ptr];
    ld.param.u64 %rd6, [children_ptr];
    ld.param.u64 %rd7, [bucket_words_ptr];
    ld.param.u64 %rd8, [bucket_rows_ptr];
    ld.param.u64 %rd9, [output_pairs_ptr];
    ld.param.u64 %rd10, [output_count_ptr];
    ld.param.u32 %r1, [bucket_index];
    ld.param.u64 %rd11, [first_pair];
    ld.param.u64 %rd12, [pair_count];

    mov.u32 %r2, %tid.x;
    mov.u32 %r3, %ctaid.x;
    mov.u32 %r4, %ntid.x;
    mad.lo.u32 %r5, %r3, %r4, %r2;
    cvt.u64.u32 %rd13, %r5;
    setp.ge.u64 %p1, %rd13, %rd12;
    @%p1 bra DONE;

    add.u64 %rd14, %rd11, %rd13;
    mul.wide.u32 %rd15, %r1, 20;
    add.u64 %rd16, %rd7, %rd15;
    ld.global.u32 %r6, [%rd16+0];
    ld.global.u32 %r7, [%rd16+4];
    ld.global.u32 %r8, [%rd16+8];

    cvt.u64.u32 %rd17, %r8;
    mov.u64 %rd18, 0;
    mov.u64 %rd19, %rd14;
    sub.u64 %rd20, %rd17, 1;

PAIR_DECODE_LOOP:
    setp.lt.u64 %p2, %rd19, %rd20;
    @%p2 bra PAIR_DECODE_DONE;
    sub.u64 %rd19, %rd19, %rd20;
    add.u64 %rd18, %rd18, 1;
    sub.u64 %rd20, %rd20, 1;
    bra PAIR_DECODE_LOOP;

PAIR_DECODE_DONE:
    add.u64 %rd21, %rd18, 1;
    add.u64 %rd21, %rd21, %rd19;
    cvt.u64.u32 %rd22, %r7;
    add.u64 %rd23, %rd22, %rd18;
    add.u64 %rd24, %rd22, %rd21;
    shl.b64 %rd23, %rd23, 2;
    shl.b64 %rd24, %rd24, 2;
    add.u64 %rd25, %rd8, %rd23;
    add.u64 %rd26, %rd8, %rd24;
    ld.global.u32 %r9, [%rd25];
    ld.global.u32 %r10, [%rd26];

    mul.wide.u32 %rd27, %r9, 4;
    mul.wide.u32 %rd28, %r10, 4;
    add.u64 %rd29, %rd5, %rd27;
    add.u64 %rd30, %rd5, %rd28;
    ld.global.u32 %r11, [%rd29];
    ld.global.u32 %r12, [%rd30];
    setp.ne.u32 %p3, %r11, %r12;
    @%p3 bra DONE;

    add.u64 %rd31, %rd2, %rd27;
    add.u64 %rd32, %rd2, %rd28;
    ld.global.u32 %r13, [%rd31];
    ld.global.u32 %r14, [%rd32];
    setp.ne.u32 %p4, %r13, %r14;
    @%p4 bra DONE;

    add.u64 %rd33, %rd4, %rd27;
    add.u64 %rd34, %rd4, %rd28;
    ld.global.u32 %r15, [%rd33];
    ld.global.u32 %r16, [%rd34];
    setp.ne.u32 %p5, %r15, %r16;
    @%p5 bra DONE;

    add.u64 %rd35, %rd3, %rd27;
    add.u64 %rd36, %rd3, %rd28;
    ld.global.u32 %r17, [%rd35];
    ld.global.u32 %r18, [%rd36];
    mov.u32 %r19, 0;

CHILD_LOOP:
    setp.ge.u32 %p6, %r19, %r15;
    @%p6 bra CHILD_DONE;
    add.u32 %r20, %r17, %r19;
    add.u32 %r21, %r18, %r19;
    mul.wide.u32 %rd37, %r20, 4;
    mul.wide.u32 %rd38, %r21, 4;
    add.u64 %rd39, %rd6, %rd37;
    add.u64 %rd40, %rd6, %rd38;
    ld.global.u32 %r22, [%rd39];
    ld.global.u32 %r23, [%rd40];
    setp.ne.u32 %p7, %r22, %r23;
    @%p7 bra DONE;
    add.u32 %r19, %r19, 1;
    bra CHILD_LOOP;

CHILD_DONE:
    add.u64 %rd41, %rd1, %rd27;
    add.u64 %rd42, %rd1, %rd28;
    ld.global.u32 %r24, [%rd41];
    ld.global.u32 %r25, [%rd42];
    setp.eq.u32 %p8, %r24, %r25;
    @%p8 bra DONE;
    setp.lt.u32 %p9, %r25, %r24;
    selp.u32 %r26, %r25, %r24, %p9;
    selp.u32 %r27, %r24, %r25, %p9;

    mov.u64 %rd43, 1;
    atom.global.add.u64 %rd44, [%rd10], %rd43;
    shl.b64 %rd45, %rd44, 3;
    add.u64 %rd46, %rd9, %rd45;
    st.global.u32 [%rd46+0], %r26;
    st.global.u32 [%rd46+4], %r27;

DONE:
    ret;
}}
"#,
        entry = CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY,
    );
    Ok(CudaEGraphStructuralEquivalenceKernelPtx {
        target_sm,
        ptx_version,
        entry_name: CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY,
        parameter_count: CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT,
        bucket_record_words: CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS,
        source,
    })
}

/// Plan deterministic CUDA union/compaction work for exact e-graph structural
/// equivalence output.
///
/// The discovery kernel can emit duplicate, reversed, and transitive merge
/// edges. This planner canonicalizes that output into unique `(left, right)`
/// pairs, computes stable minimum-id representatives for each connected merge
/// component, and partitions both union and rewrite phases into bounded CUDA
/// launch waves.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError`] if launch dimensions are invalid or
/// count arithmetic cannot be represented.
/// Emit PTX for applying canonical e-class rewrites to packed e-graph rows.
pub fn cuda_egraph_canonical_rewrite_kernel_ptx(
    target_sm: u32,
) -> Result<CudaEGraphCanonicalRewriteKernelPtx, CudaEGraphKernelPlanError> {
    let ptx_version = cuda_egraph_ptx_version(target_sm)?;
    let source = format!(
        r#".version {ptx_version}
.target sm_{target_sm}
.address_size 64

.visible .entry {entry}(
    .param .u64 row_eclass_ids_ptr,
    .param .u64 children_ptr,
    .param .u64 rewrite_words_ptr,
    .param .u32 rewrite_count,
    .param .u32 row_count,
    .param .u32 child_count,
    .param .u64 first_item
)
{{
    .reg .pred %p<8>;
    .reg .b32 %r<18>;
    .reg .b64 %rd<18>;

    ld.param.u64 %rd1, [row_eclass_ids_ptr];
    ld.param.u64 %rd2, [children_ptr];
    ld.param.u64 %rd3, [rewrite_words_ptr];
    ld.param.u32 %r1, [rewrite_count];
    ld.param.u32 %r2, [row_count];
    ld.param.u32 %r3, [child_count];
    ld.param.u64 %rd4, [first_item];

    mov.u32 %r4, %ctaid.x;
    mov.u32 %r5, %ntid.x;
    mov.u32 %r6, %tid.x;
    mad.lo.u32 %r7, %r4, %r5, %r6;
    cvt.u64.u32 %rd5, %r7;
    add.u64 %rd6, %rd4, %rd5;

    cvt.u64.u32 %rd7, %r2;
    cvt.u64.u32 %rd8, %r3;
    add.u64 %rd9, %rd7, %rd8;
    setp.ge.u64 %p0, %rd6, %rd9;
    @%p0 ret;
    setp.eq.u32 %p1, %r1, 0;
    @%p1 ret;

    setp.lt.u64 %p2, %rd6, %rd7;
    @%p2 bra ROW_ITEM;
    sub.u64 %rd10, %rd6, %rd7;
    shl.b64 %rd11, %rd10, 2;
    add.u64 %rd12, %rd2, %rd11;
    bra LOAD_VALUE;

ROW_ITEM:
    shl.b64 %rd11, %rd6, 2;
    add.u64 %rd12, %rd1, %rd11;

LOAD_VALUE:
    ld.global.u32 %r8, [%rd12];
    mov.u32 %r9, 0;
    mov.u32 %r10, %r1;

BSEARCH:
    setp.ge.u32 %p3, %r9, %r10;
    @%p3 bra CHECK_MATCH;
    add.u32 %r11, %r9, %r10;
    shr.u32 %r11, %r11, 1;
    mul.wide.u32 %rd13, %r11, 8;
    add.u64 %rd14, %rd3, %rd13;
    ld.global.u32 %r12, [%rd14];
    setp.lt.u32 %p4, %r12, %r8;
    @%p4 bra MOVE_LO;
    mov.u32 %r10, %r11;
    bra BSEARCH;

MOVE_LO:
    add.u32 %r9, %r11, 1;
    bra BSEARCH;

CHECK_MATCH:
    setp.ge.u32 %p5, %r9, %r1;
    @%p5 ret;
    mul.wide.u32 %rd15, %r9, 8;
    add.u64 %rd16, %rd3, %rd15;
    ld.global.u32 %r13, [%rd16];
    setp.ne.u32 %p6, %r13, %r8;
    @%p6 ret;
    ld.global.u32 %r14, [%rd16+4];
    st.global.u32 [%rd12], %r14;
    ret;
}}
"#,
        entry = CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_ENTRY,
    );
    Ok(CudaEGraphCanonicalRewriteKernelPtx {
        target_sm,
        ptx_version,
        entry_name: CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_ENTRY,
        parameter_count: CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT,
        rewrite_record_words: CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS,
        source,
    })
}

/// Generate PTX for refreshing structural row signatures after resident
/// canonical rewrites mutate row and child e-class ids.
///
/// The emitted mix mirrors `vyre-foundation::optimizer::eqsat_gpu` exactly:
/// seed `0xA24B_AED4`, then mix language-op id, child count, and each child
/// e-class id using the same rotate/multiply avalanche. Keeping this column
/// fresh is what allows later GPU e-graph rounds to discover duplicates that
/// only become visible after prior canonicalization.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError::InvalidPtxTarget`] when `target_sm` is
/// zero.
/// Emit PTX for refreshing row signatures after canonical e-class rewrites.
pub fn cuda_egraph_signature_refresh_kernel_ptx(
    target_sm: u32,
) -> Result<CudaEGraphSignatureRefreshKernelPtx, CudaEGraphKernelPlanError> {
    let ptx_version = cuda_egraph_ptx_version(target_sm)?;
    let source = format!(
        r#".version {ptx_version}
.target sm_{target_sm}
.address_size 64

.visible .entry {entry}(
    .param .u64 row_language_op_ids_ptr,
    .param .u64 row_children_offsets_ptr,
    .param .u64 row_children_lens_ptr,
    .param .u64 row_signatures_ptr,
    .param .u64 children_ptr,
    .param .u32 row_count,
    .param .u64 first_row
)
{{
    .reg .pred %p<4>;
    .reg .b32 %r<32>;
    .reg .b64 %rd<24>;

    ld.param.u64 %rd1, [row_language_op_ids_ptr];
    ld.param.u64 %rd2, [row_children_offsets_ptr];
    ld.param.u64 %rd3, [row_children_lens_ptr];
    ld.param.u64 %rd4, [row_signatures_ptr];
    ld.param.u64 %rd5, [children_ptr];
    ld.param.u32 %r1, [row_count];
    ld.param.u64 %rd6, [first_row];

    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.u32 %r5, %r2, %r3, %r4;
    cvt.u64.u32 %rd7, %r5;
    add.u64 %rd8, %rd6, %rd7;
    cvt.u64.u32 %rd9, %r1;
    setp.ge.u64 %p0, %rd8, %rd9;
    @%p0 ret;

    shl.b64 %rd10, %rd8, 2;
    add.u64 %rd11, %rd1, %rd10;
    add.u64 %rd12, %rd2, %rd10;
    add.u64 %rd13, %rd3, %rd10;
    ld.global.u32 %r6, [%rd11];
    ld.global.u32 %r7, [%rd12];
    ld.global.u32 %r8, [%rd13];

    mov.u32 %r30, 0x9E3779B9;
    mov.u32 %r31, 0x85EBCA6B;
    mov.u32 %r9, 0xA24BAED4;

    add.u32 %r10, %r6, %r30;
    shl.b32 %r11, %r9, 6;
    add.u32 %r10, %r10, %r11;
    shr.u32 %r12, %r9, 2;
    add.u32 %r10, %r10, %r12;
    xor.b32 %r13, %r9, %r10;
    shl.b32 %r14, %r13, 13;
    shr.u32 %r15, %r13, 19;
    or.b32 %r16, %r14, %r15;
    mul.lo.u32 %r9, %r16, %r31;

    add.u32 %r10, %r8, %r30;
    shl.b32 %r11, %r9, 6;
    add.u32 %r10, %r10, %r11;
    shr.u32 %r12, %r9, 2;
    add.u32 %r10, %r10, %r12;
    xor.b32 %r13, %r9, %r10;
    shl.b32 %r14, %r13, 13;
    shr.u32 %r15, %r13, 19;
    or.b32 %r16, %r14, %r15;
    mul.lo.u32 %r9, %r16, %r31;

    mov.u32 %r17, 0;

CHILD_LOOP:
    setp.ge.u32 %p1, %r17, %r8;
    @%p1 bra STORE_SIGNATURE;
    add.u32 %r18, %r7, %r17;
    mul.wide.u32 %rd14, %r18, 4;
    add.u64 %rd15, %rd5, %rd14;
    ld.global.u32 %r19, [%rd15];

    add.u32 %r10, %r19, %r30;
    shl.b32 %r11, %r9, 6;
    add.u32 %r10, %r10, %r11;
    shr.u32 %r12, %r9, 2;
    add.u32 %r10, %r10, %r12;
    xor.b32 %r13, %r9, %r10;
    shl.b32 %r14, %r13, 13;
    shr.u32 %r15, %r13, 19;
    or.b32 %r16, %r14, %r15;
    mul.lo.u32 %r9, %r16, %r31;
    add.u32 %r17, %r17, 1;
    bra CHILD_LOOP;

STORE_SIGNATURE:
    add.u64 %rd16, %rd4, %rd10;
    st.global.u32 [%rd16], %r9;
    ret;
}}
"#,
        entry = CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_ENTRY,
    );
    Ok(CudaEGraphSignatureRefreshKernelPtx {
        target_sm,
        ptx_version,
        entry_name: CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_ENTRY,
        parameter_count: CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT,
        source,
    })
}

fn cuda_egraph_ptx_version(target_sm: u32) -> Result<&'static str, CudaEGraphKernelPlanError> {
    if target_sm == 0 {
        return Err(CudaEGraphKernelPlanError::InvalidPtxTarget { target_sm });
    }
    Ok(match target_sm {
        120.. => "8.7",
        100..=119 => "8.6",
        90..=99 => "8.0",
        _ => "8.5",
    })
}
