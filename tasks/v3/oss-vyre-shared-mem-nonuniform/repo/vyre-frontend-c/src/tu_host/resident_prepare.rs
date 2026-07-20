use super::*;
#[cfg(feature = "cpu-oracle")]
/// Prepare a translation unit with the host reference path used for residency
/// parity checks.
pub fn reference_prepare_resident_translation_unit_source(
    tu_path: &Path,
    raw: &str,
    options: &VyreCompileOptions,
) -> Result<String, String> {
    let spliced = splice_line_continuations(raw);
    let prefixed = apply_cli_source_prefix(&spliced, options)?;
    let mut stack = Vec::new();
    let expanded = expand_local_includes_with_search_dirs(
        &prefixed,
        tu_path,
        &options.include_dirs,
        &options.quote_include_dirs,
        &options.system_include_dirs,
        &options.after_include_dirs,
        !options.disable_system_include_dirs,
        options.system_include_sysroot.as_deref(),
        0,
        &mut stack,
    )?;
    Ok(reference_expand_preprocessor_macros(&expanded))
}

/// Production resident frontend prep.
///
/// Acquires the CUDA-first dispatch backend and runs preprocessing through
/// GPU kernels. Host work is limited to file I/O and include path resolution.
pub fn prepare_resident_translation_unit_source(
    tu_path: &Path,
    raw: &str,
    options: &VyreCompileOptions,
) -> Result<String, String> {
    prepare_resident_translation_unit_source_gpu(tu_path, raw, options)
}

/// GPU-backed resident frontend prep implementation.
///
/// Acquires the preferred dispatch backend, wraps the existing
/// `search_include_file`/`search_system_include_file` logic in an
/// [`gpu_pipeline::IncludeLoader`], and runs the full
/// `gpu_preprocess_translation_unit` chain (filter → lex → classify →
/// payload extract → conditional walk → recursive include).
///
/// Returns a UTF-8 `String` of the GPU-preprocessed bytes.
///
/// # Errors
///
/// Returns the dispatcher / loader error verbatim if any stage fails.
pub fn prepare_resident_translation_unit_source_gpu(
    tu_path: &Path,
    raw: &str,
    options: &VyreCompileOptions,
) -> Result<String, String> {
    use gpu_pipeline::{gpu_preprocess_translation_unit, IncludeLoader as _, MacroDef};

    reject_mixed_macro_transport(options)?;
    let trace = std::env::var_os("VYRE_STAGE_TRACE").is_some();
    let stage_start = std::time::Instant::now();
    let mut last_t = stage_start;
    let mut log = |label: &str| {
        if trace {
            let now = std::time::Instant::now();
            let stage = now.duration_since(last_t).as_micros();
            let total = now.duration_since(stage_start).as_micros();
            eprintln!(
                "[stage-trace] +{stage}us (total {total}us): resident-prep {} {label}",
                tu_path.display()
            );
            last_t = now;
        }
    };

    let spliced = splice_line_continuations(raw);
    log("splice line continuations");
    let prefixed = apply_forced_include_prefix(&spliced, options)?;
    log("apply forced-include prefix");
    let cache_key = resident_prep_key(tu_path, prefixed.as_bytes(), options)?;
    log("build cache key");
    let cache = resident_prep_cache();
    let cached_deps = {
        let mut guard = cache
            .lock()
            .map_err(|_| "vyre-frontend-c: resident preprocessor cache poisoned".to_string())?;
        lookup_resident_prep_cache_deps(&mut guard, &cache_key)
    };
    if let Some(deps) = cached_deps {
        log("cache lookup hit");
        if resident_prep_deps_valid(&deps)? {
            log("cache deps valid");
            let guard = cache.lock().map_err(|_| {
                "vyre-frontend-c: resident preprocessor cache poisoned while returning cached source"
                    .to_string()
            })?;
            if let Some(source) = clone_resident_prep_cache_source(&guard, &cache_key) {
                return Ok(source);
            }
            log("cache source raced with eviction");
        } else {
            let mut guard = cache.lock().map_err(|_| {
                "vyre-frontend-c: resident preprocessor cache poisoned while removing stale entry"
                    .to_string()
            })?;
            remove_stale_resident_prep_cache_entry(&mut guard, &cache_key);
        }
    } else {
        log("cache lookup miss");
    }

    let backend = resident_preprocessor_backend()?;
    log("acquire backend");
    let dispatcher = CachedResidentDispatcher(backend.as_ref());
    let loader = ResidentIncludeLoader::new(
        &options.include_dirs,
        &options.quote_include_dirs,
        &options.system_include_dirs,
        &options.after_include_dirs,
        !options.disable_system_include_dirs,
        options.system_include_sysroot.as_deref(),
    )?;
    log("create include loader");
    let mut active_macros: Vec<MacroDef> = cli_macro_defs(options);
    for imacro in &options.imacro_files {
        let imacro_name = imacro.to_str().ok_or_else(|| {
            format!(
                "vyre-frontend-c: -imacros operand {} is not valid UTF-8. Fix: pass macro import paths as UTF-8; lossy include lookup is forbidden.",
                imacro.display()
            )
        })?;
        let Some((imacro_path, imacro_bytes)) =
            loader.load(imacro_name.as_bytes(), false, false, tu_path)?
        else {
            return Err(format!(
                "vyre-frontend-c: -imacros operand {} resolved to no file. Fix: pass a valid macro import file or include root.",
                imacro.display()
            ));
        };
        let imacro_res = gpu_preprocess_translation_unit(
            &dispatcher,
            &loader,
            &imacro_path,
            &imacro_bytes,
            &active_macros,
        )?;
        active_macros = imacro_res.macros;
    }

    let res = gpu_preprocess_translation_unit(
        &dispatcher,
        &loader,
        tu_path,
        prefixed.as_bytes(),
        &active_macros,
    )?;
    log("gpu preprocess translation unit");
    let source = String::from_utf8(res.bytes).map_err(|error| {
        format!(
            "vyre-frontend-c: GPU preprocessor emitted non-UTF-8 source bytes at offset {}. Fix: preserve preprocessed translation units as bytes before parsing or reject the input encoding before GPU preprocessing; lossy replacement is forbidden.",
            error.utf8_error().valid_up_to()
        )
    })?;
    let deps = loader.dependency_signature()?;
    log("dependency signature");
    let mut guard = cache.lock().map_err(|_| {
        "vyre-frontend-c: resident preprocessor cache poisoned while inserting".to_string()
    })?;
    insert_resident_prep_cache(
        &mut guard,
        cache_key,
        ResidentPrepEntry {
            source: source.clone(),
            deps: std::sync::Arc::from(deps.into_boxed_slice()),
        },
    );
    log("cache insert");
    Ok(source)
}
