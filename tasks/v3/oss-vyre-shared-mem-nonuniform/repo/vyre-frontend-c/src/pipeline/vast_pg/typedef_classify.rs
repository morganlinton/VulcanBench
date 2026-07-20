use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct TypedefClassifyScratch {
    annotate_outputs: Vec<Vec<u8>>,
    classify_outputs: Vec<Vec<u8>>,
    fused_outputs: Vec<Vec<u8>>,
}

thread_local! {
    static TYPEDEF_CLASSIFY_SCRATCH: RefCell<TypedefClassifyScratch> =
        RefCell::new(TypedefClassifyScratch::default());
}

pub(super) fn classify_typedef_vast(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    decl_context_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    readback_terminal_outputs: bool,
    has_typedef_keyword: bool,
    global_typedef_hashes: Option<&[u8]>,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
) -> Result<Vec<u8>, String> {
    TYPEDEF_CLASSIFY_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "VAST typedef/classify dispatch scratch was re-entered on the same thread. Fix: call typedef classification from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        classify_typedef_vast_with_scratch(
            backend,
            path,
            scoped_vast_blob,
            decl_context_blob,
            haystack,
            haystack_len,
            vast_count,
            packed_haystack,
            readback_terminal_outputs,
            has_typedef_keyword,
            global_typedef_hashes,
            cfg,
            log,
            &mut scratch,
        )
    })
}

fn classify_typedef_vast_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    decl_context_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    readback_terminal_outputs: bool,
    has_typedef_keyword: bool,
    global_typedef_hashes: Option<&[u8]>,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut TypedefClassifyScratch,
) -> Result<Vec<u8>, String> {
    if has_typedef_keyword && light_runtime_fusion_enabled(readback_terminal_outputs, vast_count) {
        return classify_typedef_vast_fused_or_unfused(
            backend,
            path,
            scoped_vast_blob,
            decl_context_blob,
            haystack,
            haystack_len,
            vast_count,
            packed_haystack,
            global_typedef_hashes,
            cfg,
            log,
            scratch,
        );
    }
    classify_typedef_vast_unfused(
        backend,
        path,
        scoped_vast_blob,
        decl_context_blob,
        haystack,
        haystack_len,
        vast_count,
        packed_haystack,
        has_typedef_keyword,
        global_typedef_hashes,
        cfg,
        log,
        scratch,
    )
}

fn classify_typedef_vast_unfused(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    decl_context_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    has_typedef_keyword: bool,
    global_typedef_hashes: Option<&[u8]>,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut TypedefClassifyScratch,
) -> Result<Vec<u8>, String> {
    if std::env::var_os("VYRE_STAGE_TRACE").is_some() {
        eprintln!(
            "[stage-trace] typedef/classify mode=unfused vast_count={vast_count} haystack_len={haystack_len} packed_haystack={packed_haystack} global_typedefs={}",
            global_typedef_hashes.is_some()
        );
    }
    cfg.label = Some(format!("vyre-frontend-c vast-typedefs {}", path.display()));
    let global_typedef_count = global_typedef_hash_count(global_typedef_hashes)?;
    let annotated_vast = if !has_typedef_keyword && global_typedef_count == 0 {
        if std::env::var_os("VYRE_STAGE_TRACE").is_some() {
            eprintln!(
                "[stage-trace] typedef/classify skipped annotation because no typedef keyword is present"
            );
        }
        scoped_vast_blob.to_vec()
    } else {
        let annotate_key = super::stage_pipeline_cache_key(
            "c11_annotate_typedef_names",
            &[
                haystack_len.max(1) as u64,
                vast_count.max(1) as u64,
                packed_haystack as u64,
                global_typedef_count as u64,
            ],
        );
        let annotate_normal_inputs = [scoped_vast_blob, haystack, decl_context_blob];
        let annotate_global_inputs;
        let annotate_inputs: &[&[u8]] = if let Some(global_typedef_hashes) = global_typedef_hashes {
            annotate_global_inputs = [scoped_vast_blob, global_typedef_hashes];
            &annotate_global_inputs
        } else {
            &annotate_normal_inputs
        };
        scratch.annotate_outputs.clear();
        super::dispatch_borrowed_stage_cached_into(
            backend,
            annotate_key,
            || {
                let annot_prog = annotation_program(
                    haystack_len,
                    vast_count,
                    packed_haystack,
                    global_typedef_count,
                );
                let annot_prog =
                    super::buffers::mark_program_outputs(annot_prog, &["annotated_vast"]);
                super::validate_internal_stage(&annot_prog, "c11_annotate_typedef_names")?;
                Ok(annot_prog)
            },
            annotate_inputs,
            cfg,
            &mut scratch.annotate_outputs,
        )
        .map_err(|error| format!("c11_annotate_typedef_names dispatch failed: {error}"))?;
        log("dispatch c11_annotate_typedef_names");
        super::buffers::take_exact_output(
            "c11_annotate_typedef_names",
            &mut scratch.annotate_outputs,
        )?
    };
    cfg.label = Some(format!("vyre-frontend-c vast-classify {}", path.display()));
    let classify_key = super::stage_pipeline_cache_key(
        "c11_classify_vast_node_kinds",
        &[vast_count.max(1) as u64, 1],
    );
    if std::env::var_os("VYRE_STAGE_TRACE").is_some() {
        eprintln!("[stage-trace] typedef/classify dispatching classify vast_count={vast_count}");
    }
    let classify_inputs = [annotated_vast.as_slice(), decl_context_blob];
    let classify_prog =
        super::buffers::mark_program_outputs(classify_program(vast_count), &["typed_vast_nodes"]);
    super::validate_internal_stage(&classify_prog, "c11_classify_vast_node_kinds")?;
    let previous_workgroup_override = cfg.workgroup_override;
    cfg.workgroup_override = Some(classify_prog.workgroup_size());
    scratch.classify_outputs.clear();
    let classify_dispatch = super::dispatch_borrowed_stage_cached_into(
        backend,
        classify_key,
        || Ok(classify_prog),
        &classify_inputs,
        cfg,
        &mut scratch.classify_outputs,
    );
    cfg.workgroup_override = previous_workgroup_override;
    classify_dispatch
        .map_err(|error| format!("c11_classify_vast_node_kinds dispatch failed: {error}"))?;
    log("dispatch c11_classify_vast_node_kinds");
    super::buffers::take_exact_output(
        "c11_classify_vast_node_kinds",
        &mut scratch.classify_outputs,
    )
}

fn classify_typedef_vast_fused_or_unfused(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    decl_context_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    global_typedef_hashes: Option<&[u8]>,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut TypedefClassifyScratch,
) -> Result<Vec<u8>, String> {
    let global_typedef_count = global_typedef_hash_count(global_typedef_hashes)?;
    let annot_prog = super::buffers::mark_program_outputs(
        annotation_program(
            haystack_len,
            vast_count,
            packed_haystack,
            global_typedef_count,
        ),
        &["annotated_vast"],
    );
    super::validate_internal_stage(&annot_prog, "c11_annotate_typedef_names")?;
    let classify_prog =
        super::buffers::mark_program_outputs(classify_program(vast_count), &["typed_vast_nodes"]);
    super::validate_internal_stage(&classify_prog, "c11_classify_vast_node_kinds")?;
    match vyre_foundation::execution_plan::fusion::fuse_programs(&[
        annot_prog.clone(),
        classify_prog.clone(),
    ]) {
        Ok(fused) => {
            cfg.label = Some(format!(
                "vyre-frontend-c vast-typedefs+classify {}",
                path.display()
            ));
            let fusion_normal_inputs = [scoped_vast_blob, haystack, decl_context_blob];
            let fusion_global_inputs;
            let fusion_inputs: &[&[u8]] = if let Some(global_typedef_hashes) = global_typedef_hashes
            {
                fusion_global_inputs = [scoped_vast_blob, global_typedef_hashes, decl_context_blob];
                &fusion_global_inputs
            } else {
                &fusion_normal_inputs
            };
            scratch.fused_outputs.clear();
            match super::dispatch_borrowed_cached_into(
                backend,
                &fused,
                fusion_inputs,
                cfg,
                &mut scratch.fused_outputs,
            ) {
                Ok(()) => {
                    if scratch.fused_outputs.is_empty() {
                        return Err(
                            "fused VAST typedef/classify: missing typed VAST output".to_string()
                        );
                    }
                    log("dispatch fused typedef/classify");
                    let typed_idx = scratch.fused_outputs.len() - 1;
                    let mut typed_vast = Vec::new();
                    mem::swap(&mut typed_vast, &mut scratch.fused_outputs[typed_idx]);
                    Ok(typed_vast)
                }
                Err(error) => {
                    if std::env::var_os("VYRE_STAGE_TRACE").is_some() {
                        eprintln!(
                            "[stage-trace] fused VAST typedef/classify rejected by backend; running unfused GPU stages on the same backend: {error}"
                        );
                    }
                    classify_typedef_vast_unfused(
                        backend,
                        path,
                        scoped_vast_blob,
                        decl_context_blob,
                        haystack,
                        haystack_len,
                        vast_count,
                        packed_haystack,
                        true,
                        global_typedef_hashes,
                        cfg,
                        log,
                        scratch,
                    )
                }
            }
        }
        Err(e) => {
            eprintln!(
                "[vyre] fuse_programs failed for typedef+classify (path={}); running unfused GPU stages: {e}",
                path.display()
            );
            classify_typedef_vast_unfused(
                backend,
                path,
                scoped_vast_blob,
                decl_context_blob,
                haystack,
                haystack_len,
                vast_count,
                packed_haystack,
                true,
                global_typedef_hashes,
                cfg,
                log,
                scratch,
            )
        }
    }
}

fn annotation_program(
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    global_typedef_count: u32,
) -> vyre::ir::Program {
    if global_typedef_count != 0 {
        c11_annotate_global_typedef_names_fast(
            "vast_nodes",
            "global_typedef_hashes",
            Expr::u32(vast_count.max(1)),
            Expr::u32(global_typedef_count),
            "annotated_vast",
        )
    } else if packed_haystack {
        c11_annotate_typedef_names_precomputed_context_packed_haystack(
            "vast_nodes",
            "haystack",
            "decl_contexts",
            Expr::u32(haystack_len.max(1)),
            Expr::u32(vast_count.max(1)),
            "annotated_vast",
        )
    } else {
        c11_annotate_typedef_names_precomputed_context(
            "vast_nodes",
            "haystack",
            "decl_contexts",
            Expr::u32(haystack_len.max(1)),
            Expr::u32(vast_count.max(1)),
            "annotated_vast",
        )
    }
}

fn classify_program(vast_count: u32) -> vyre::ir::Program {
    c11_classify_annotated_vast_node_kinds_precomputed_context(
        "annotated_vast",
        "decl_contexts",
        Expr::u32(vast_count.max(1)),
        "typed_vast_nodes",
    )
}

#[cfg(test)]
mod tests {
    use vyre_foundation::execution_plan::fusion::{FusionError, FusionOverDispatchError};

    /// The `Err(e)` arm of `classify_typedef_vast_fused_or_unfused` now calls
    /// `eprintln!(... "{e}" ...)`. This test verifies that every concrete
    /// `FusionError` variant formats to a non-empty diagnostic string, so the
    /// `eprintln!` cannot silently produce an empty message.
    #[test]
    fn fusion_error_display_is_non_empty_for_all_variants() {
        let over_dispatch = FusionError::OverDispatch(FusionOverDispatchError {
            max_arm_threads: 1024,
            fused_threads: 65536,
            fix: "split the batch",
        });
        let msg = format!("{over_dispatch}");
        assert!(
            !msg.is_empty(),
            "FusionError::OverDispatch must format to a non-empty diagnostic"
        );
        assert!(
            msg.contains("1024") && msg.contains("65536"),
            "FusionError::OverDispatch message must include thread counts; got: {msg}"
        );
    }
}
