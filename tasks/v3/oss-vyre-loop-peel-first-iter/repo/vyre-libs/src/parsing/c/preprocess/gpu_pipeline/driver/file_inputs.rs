use std::path::Path;
use std::sync::Arc;

use super::stage_trace::StageTrace;
use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::cache::{
    cached_classified_tokens, cached_payloads, classified_cache_key_from_hash,
    insert_classified_tokens, insert_payloads, load_classified_from_disk, load_payloads_from_disk,
    production_payloads_cache_key_from_hash, source_hash128, store_classified_to_disk,
    store_payloads_to_disk,
};
use crate::parsing::c::preprocess::gpu_pipeline::header_reuse::{
    header_reuse_key_from_hash, load_header_reuse, reuse_event, store_header_reuse,
    HeaderReuseEntry,
};
use crate::parsing::c::preprocess::gpu_pipeline::{ClassifiedTokens, DirectivePayload};

pub(super) struct PreparedFile {
    pub(super) classified: Arc<ClassifiedTokens>,
    pub(super) payloads: Arc<[DirectivePayload]>,
}

pub(super) fn prepare_file_inputs(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    source: &[u8],
    depth: u32,
    trace: &mut StageTrace<'_>,
) -> Result<PreparedFile, String> {
    let source_hash = source_hash128(source);
    let header_key = if depth > 0 {
        let defines_hash = run.live_defines_hash();
        Some(header_reuse_key_from_hash(
            file_path,
            source_hash,
            defines_hash,
        ))
    } else {
        None
    };
    let header_hit = header_key
        .as_ref()
        .map(|key| load_header_reuse(key).map(|entry| entry.map(|entry| (key.clone(), entry))))
        .transpose()?
        .flatten();
    let (classified, payloads) = if let Some((key, entry)) = header_hit {
        trace.log("header reuse cache hit");
        run.header_reuse_events.push(reuse_event(&key, true, false));
        (entry.classified, entry.payloads)
    } else {
        let classified_key = classified_cache_key_from_hash(file_path, source.len(), source_hash);
        let classified = if let Some(classified) = cached_classified_tokens(&classified_key)? {
            trace.log("classified cache hit");
            classified
        } else if let Some(classified) = load_classified_from_disk(&classified_key)? {
            trace.log("classified disk cache hit");
            let classified = Arc::new(classified);
            insert_classified_tokens(classified_key.clone(), Arc::clone(&classified))?;
            classified
        } else {
            let filtered = crate::parsing::c::preprocess::gpu_pipeline::gpu_pipeline_filter::gpu_filter_source_bytes_with_scratch(
                run.dispatcher,
                source,
                &mut run.filter_scratch,
            )?;
            trace.log("filter source bytes");
            let classified = Arc::new(
                crate::parsing::c::preprocess::gpu_pipeline::tokenization::gpu_tokenize_and_classify_with_scratch(
                    run.dispatcher,
                    &filtered.bytes,
                    &mut run.tokenization_scratch,
                )?,
            );
            trace.log("tokenize and classify");
            store_classified_to_disk(&classified_key, classified.as_ref())?;
            insert_classified_tokens(classified_key, Arc::clone(&classified))?;
            classified
        };
        let payloads_key =
            production_payloads_cache_key_from_hash(file_path, source.len(), source_hash);
        let payloads = if let Some(payloads) = cached_payloads(&payloads_key)? {
            trace.log("payloads cache hit");
            payloads
        } else if let Some(payloads) = load_payloads_from_disk(&payloads_key)? {
            trace.log("payloads disk cache hit");
            let payloads = Arc::from(payloads.into_boxed_slice());
            insert_payloads(payloads_key.clone(), Arc::clone(&payloads))?;
            payloads
        } else {
            let payloads = crate::parsing::c::preprocess::gpu_pipeline::directives::gpu_extract_directive_payloads_for_driver_with_scratch(
                run.dispatcher,
                &classified,
                &mut run.directive_extraction_scratch,
            )?;
            trace.log("extract directive payloads");
            store_payloads_to_disk(&payloads_key, &payloads)?;
            let payloads = Arc::from(payloads.into_boxed_slice());
            insert_payloads(payloads_key, Arc::clone(&payloads))?;
            payloads
        };
        if let Some(key) = header_key {
            store_header_reuse(
                key.clone(),
                HeaderReuseEntry {
                    classified: Arc::clone(&classified),
                    payloads: Arc::clone(&payloads),
                },
            )?;
            run.header_reuse_events.push(reuse_event(&key, false, true));
        }
        (classified, payloads)
    };
    Ok(PreparedFile {
        classified,
        payloads,
    })
}
