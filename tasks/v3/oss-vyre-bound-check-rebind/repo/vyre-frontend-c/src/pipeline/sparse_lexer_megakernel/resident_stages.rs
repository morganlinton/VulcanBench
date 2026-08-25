use super::*;
use smallvec::SmallVec;

pub(super) fn workgroup_prefix_scan_u32(input: &str, output: &str, len: u32) -> Program {
    prefix_scan(input, output, len.max(1), ScanKind::InclusiveSum)
}

pub(super) fn dispatch_sparse_lexer_cached_stages_resident(
    backend: &dyn VyreBackend,
    haystack_bytes: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    no_literal_backscan: bool,
    scratch: &mut SparseLexerMegakernelScratch,
) -> Result<SparseLexerMegakernelOutput, String> {
    let mut cleanup = SmallVec::<[ResidentBlob; 16]>::new();
    let result = (|| -> Result<SparseLexerMegakernelOutput, String> {
        config.label = Some(format!("{label} sparse-lexer-stage-sparse-resident"));
        let sparse_key = stage_pipeline_cache_key(
            if no_literal_backscan {
                "syntax_sparse_lexer_stage_sparse_no_literal_backscan_resident"
            } else {
                "syntax_sparse_lexer_stage_sparse_resident"
            },
            &[haystack_len as u64],
        );
        let sparse_outputs = dispatch_resident_stage_cached(
            backend,
            sparse_key,
            || {
                let sparse = if no_literal_backscan {
                    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan(
                        "haystack",
                        "sparse_types",
                        "sparse_starts",
                        "sparse_lens",
                        "sparse_flags",
                        haystack_len,
                    )
                } else {
                    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives(
                        "haystack",
                        "sparse_types",
                        "sparse_starts",
                        "sparse_lens",
                        "sparse_flags",
                        haystack_len,
                    )
                };
                let sparse = mark_output_buffers(
                    sparse,
                    &[
                        "sparse_types",
                        "sparse_starts",
                        "sparse_lens",
                        "sparse_flags",
                    ],
                );
                validate_internal_stage(&sparse, "syntax_sparse_lexer_stage_sparse_resident")?;
                Ok(sparse)
            },
            &[ResidentStageInput::Host(haystack_bytes)],
            config,
        )
        .map_err(|e| format!("{label} sparse lexer resident sparse dispatch failed: {e}"))?;
        cleanup.extend(sparse_outputs.iter().cloned());
        let mut sparse_pairs = resident_output_pairs(
            [
                "sparse_types",
                "sparse_starts",
                "sparse_lens",
                "sparse_flags",
            ],
            sparse_outputs,
            label,
            "resident sparse",
        )?;
        let sparse_types = take_resident_blob(&mut sparse_pairs, "sparse_types", label)?;
        let sparse_starts = take_resident_blob(&mut sparse_pairs, "sparse_starts", label)?;
        let sparse_lens = take_resident_blob(&mut sparse_pairs, "sparse_lens", label)?;
        let sparse_flags = take_resident_blob(&mut sparse_pairs, "sparse_flags", label)?;

        let offsets = dispatch_resident_prefix_scan_u32(
            backend,
            &sparse_flags,
            haystack_len,
            config,
            label,
            &mut cleanup,
        )
        .map_err(|e| format!("{label} sparse lexer resident scan dispatch failed: {e}"))?;

        config.label = Some(format!("{label} sparse-lexer-stage-compact-resident"));
        let compact_key = stage_pipeline_cache_key(
            "syntax_sparse_lexer_stage_compact_resident",
            &[haystack_len as u64],
        );
        let compact_outputs = dispatch_resident_stage_cached(
            backend,
            compact_key,
            || {
                let compact = mark_output_buffers(
                    c11_compact_sparse_tokens_output(
                        "sparse_types",
                        "sparse_starts",
                        "sparse_lens",
                        "offsets",
                        "out_tok_types",
                        "out_tok_starts",
                        "out_tok_lens",
                        "out_counts",
                        haystack_len,
                    ),
                    &[
                        "out_tok_types",
                        "out_tok_starts",
                        "out_tok_lens",
                        "out_counts",
                    ],
                );
                validate_internal_stage(&compact, "syntax_sparse_lexer_stage_compact_resident")?;
                Ok(compact)
            },
            &[
                ResidentStageInput::Resident(&sparse_types),
                ResidentStageInput::Resident(&sparse_starts),
                ResidentStageInput::Resident(&sparse_lens),
                ResidentStageInput::Resident(&offsets),
            ],
            config,
        )
        .map_err(|e| format!("{label} sparse lexer resident compact dispatch failed: {e}"))?;
        cleanup.extend(compact_outputs.iter().cloned());
        collect_resident_compact_lexer_output_exact_readback(
            backend,
            compact_outputs,
            [
                "out_tok_types",
                "out_tok_starts",
                "out_tok_lens",
                "out_counts",
            ],
            &mut scratch.resident_compact_outputs,
            &mut scratch.resident_count_readback,
            label,
            "resident cached stages",
        )
    })();

    let cleanup_result = free_resident_blobs(backend, cleanup)
        .map_err(|e| format!("{label} sparse lexer resident cleanup failed: {e}"));
    match (result, cleanup_result) {
        (Ok(output), Ok(())) => Ok(output),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup_error)) | (Err(_), Err(cleanup_error)) => Err(cleanup_error),
    }
}

pub(super) fn dispatch_resident_prefix_scan_u32(
    backend: &dyn VyreBackend,
    input: &ResidentBlob,
    len: u32,
    config: &mut DispatchConfig,
    label: &str,
    cleanup: &mut SmallVec<[ResidentBlob; 16]>,
) -> Result<ResidentBlob, String> {
    if len <= BLOCK_LANES {
        config.label = Some(format!("{label} sparse-lexer-stage-scan-resident"));
        let scan_key =
            stage_pipeline_cache_key("syntax_sparse_lexer_stage_scan_resident", &[len as u64]);
        let scan_outputs = dispatch_resident_stage_cached(
            backend,
            scan_key,
            || {
                let scan = mark_output_buffers(
                    prefix_scan("scan_in", "scan_out", len.max(1), ScanKind::InclusiveSum),
                    &["scan_out"],
                );
                validate_internal_stage(&scan, "syntax_sparse_lexer_stage_scan_resident")?;
                Ok(scan)
            },
            &[ResidentStageInput::Resident(input)],
            config,
        )
        .map_err(|e| format!("{label} resident prefix scan single-block dispatch failed: {e}"))?;
        cleanup.extend(scan_outputs.iter().cloned());
        let mut scan_pairs =
            resident_output_pairs(["scan_out"], scan_outputs, label, "resident scan")?;
        return take_resident_blob(&mut scan_pairs, "scan_out", label);
    }

    let num_blocks = len.div_ceil(BLOCK_LANES);
    config.label = Some(format!("{label} sparse-lexer-stage-scan-pass-a-resident"));
    let pass_a_key = stage_pipeline_cache_key(
        "syntax_sparse_lexer_stage_scan_pass_a_resident",
        &[len as u64],
    );
    let pass_a_outputs = dispatch_resident_stage_cached(
        backend,
        pass_a_key,
        || {
            let pass_a = pass_a_local_scan(
                "scan_in",
                "scan_partials",
                "scan_block_totals",
                len,
                num_blocks,
            );
            validate_internal_stage(&pass_a, "syntax_sparse_lexer_stage_scan_pass_a_resident")?;
            Ok(pass_a)
        },
        &[ResidentStageInput::Resident(input)],
        config,
    )
    .map_err(|e| format!("{label} resident prefix scan pass A dispatch failed: {e}"))?;
    cleanup.extend(pass_a_outputs.iter().cloned());
    let mut pass_a_pairs = resident_output_pairs(
        ["scan_partials", "scan_block_totals"],
        pass_a_outputs,
        label,
        "resident scan pass A",
    )?;
    let partials = take_resident_blob(&mut pass_a_pairs, "scan_partials", label)?;
    let block_totals = take_resident_blob(&mut pass_a_pairs, "scan_block_totals", label)?;

    let scanned_block_totals = dispatch_resident_prefix_scan_u32(
        backend,
        &block_totals,
        num_blocks,
        config,
        label,
        cleanup,
    )?;

    config.label = Some(format!("{label} sparse-lexer-stage-scan-pass-c-resident"));
    let pass_c_key = stage_pipeline_cache_key(
        "syntax_sparse_lexer_stage_scan_pass_c_resident",
        &[len as u64],
    );
    let pass_c_outputs = dispatch_resident_stage_cached(
        backend,
        pass_c_key,
        || {
            let pass_c = mark_output_buffers(
                pass_c_broadcast_offsets(
                    "scan_partials",
                    "scan_block_totals_scanned",
                    "scan_out",
                    len,
                    num_blocks,
                ),
                &["scan_out"],
            );
            validate_internal_stage(&pass_c, "syntax_sparse_lexer_stage_scan_pass_c_resident")?;
            Ok(pass_c)
        },
        &[
            ResidentStageInput::Resident(&partials),
            ResidentStageInput::Resident(&scanned_block_totals),
        ],
        config,
    )
    .map_err(|e| format!("{label} resident prefix scan pass C dispatch failed: {e}"))?;
    cleanup.extend(pass_c_outputs.iter().cloned());
    let mut pass_c_pairs =
        resident_output_pairs(["scan_out"], pass_c_outputs, label, "resident scan pass C")?;
    take_resident_blob(&mut pass_c_pairs, "scan_out", label)
}
