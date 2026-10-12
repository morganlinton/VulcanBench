//! Execute one or more bench cases. Audit-fix A29 split this module by
//! concern: stats helpers in `stats.rs`, the per-case driver in `run_case.rs`,
//! sample collection in `collect.rs`, metric-key plumbing in `metric_keys.rs`,
//! and report formatting in `report.rs`.

mod collect;
mod metric_keys;
mod report;
mod run_case;
mod stats;

pub use report::print_report;

use run_case::run_case;

use crate::api::case::{BenchContext, BenchError, Correctness, PerformanceContract};
use crate::api::suite::SuiteKind;
use crate::probes::environment::{capture_environment, EnvironmentData};
use crate::registry::BenchRegistry;
use crate::report::json::{
    benchmark_held_out_corpus_id, CaseReport, ReportBackendProfile, ReportSchema, ReportSummary,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub backend_id: Option<String>,
    pub enforce_budgets: bool,
    pub case_ids: Vec<String>,
    pub warmup_samples: usize,
    pub measured_samples: Option<usize>,
    pub sample_timeout: Duration,
    pub determinism_runs: usize,
    pub workgroup_override: Option<[u32; 3]>,
    pub baseline_warmup_runs: usize,
    pub snapshot_on_pass: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            backend_id: None,
            enforce_budgets: false,
            case_ids: Vec::new(),
            warmup_samples: 3,
            measured_samples: None,
            sample_timeout: Duration::from_secs(30),
            determinism_runs: 1,
            workgroup_override: None,
            baseline_warmup_runs: 0,
            snapshot_on_pass: false,
        }
    }
}

pub fn run_suite(registry: &BenchRegistry, suite: SuiteKind, format: &str) {
    let config = RunConfig::default();
    let report = execute_suite(registry, suite, &config);

    // Write longitudinal tracking database
    if let Err(error) = crate::report::write_sqlite_report(&report) {
        eprintln!("Warning: failed to write longitudinal sqlite report: {error}");
    }

    // Write chrome trace for roofline analysis
    if let Err(error) = crate::report::write_chrome_trace(&report) {
        eprintln!("Warning: failed to write chrome trace report: {error}");
    }

    if let Err(error) = print_report(&report, format, false) {
        eprintln!("failed to render benchmark report: {error}");
        std::process::exit(1);
    }
    if report.summary.failed > 0 {
        std::process::exit(1);
    }
}

pub fn execute_suite(
    registry: &BenchRegistry,
    suite: SuiteKind,
    config: &RunConfig,
) -> ReportSchema {
    let environment = capture_environment().unwrap_or_else(|error| {
        eprintln!(
            "vyre-bench fatal error: benchmark environment probe failed: {error}. Fix: repair GPU/NVIDIA provenance before collecting performance evidence."
        );
        std::process::exit(1);
    });
    let started = Instant::now();
    let mut cases_report = Vec::with_capacity(registry.len());
    let mut passed = 0;
    let mut failed = 0;
    let mut total_cache_hits = 0;
    let mut total_cache_observed = 0;
    let mut selected_backend_profile = None;

    let selected_cases: Vec<_> = registry
        .iter()
        .filter(|case| {
            let selected_by_id = config
                .case_ids
                .iter()
                .any(|case_id| case.metadata().id.0 == *case_id);
            selected_by_id || (config.case_ids.is_empty() && case.active_in_suite(suite))
        })
        .collect();

    for case in selected_cases.iter().copied() {
        let meta = case.metadata();
        let requirements = case.requirements();
        if let Err(error) = validate_requirements(&environment, &requirements) {
            failed += 1;
            cases_report.push(case_failure(
                case,
                None,
                format!("Environment error: {error}"),
                case.performance_contract(),
            ));
            continue;
        }

        let preferred_backend: Arc<dyn vyre::VyreBackend> =
            match acquire_backend(config.backend_id.as_deref()) {
                Ok(backend) => backend,
                Err(error) => {
                    failed += 1;
                    cases_report.push(case_failure(
                        case,
                        None,
                        format!("Backend error: {error}"),
                        case.performance_contract(),
                    ));
                    continue;
                }
            };

        if selected_backend_profile.is_none() {
            selected_backend_profile = Some(ReportBackendProfile::from_device_profile(
                preferred_backend.device_profile(),
            ));
        }

        let mut ctx = BenchContext {
            backends: vec![],
            preferred_backend,
            compiled_pipeline: None,
            compiled_program_fingerprint: None,
            reference: crate::api::case::CpuReference {},
            optimizer: crate::api::case::OptimizerPipeline {},
            scratch: crate::api::case::ScratchPool { buffer: vec![] },
            rng: rand::SeedableRng::seed_from_u64(42),
            dispatch_config: dispatch_config(config),
            evolve_candidate: None,
            include_baseline_outputs: false,
        };

        let mut prepared = match case.prepare(&mut ctx) {
            Ok(prepared) => prepared,
            Err(error) => {
                failed += 1;
                cases_report.push(case_failure(
                    case,
                    Some(ctx.preferred_backend.id().to_string()),
                    format!("Prepare error: {error}"),
                    case.performance_contract(),
                ));
                continue;
            }
        };

        let mut compile_cache_hit = None;
        if let Some(program) = ctx
            .evolve_candidate
            .as_ref()
            .or_else(|| case.program(&prepared))
        {
            ctx.compiled_program_fingerprint = Some(program.fingerprint());
            let compile_res = (|| {
                let mut inferred_config;
                let compile_config = if ctx.dispatch_config.grid_override.is_none() {
                    inferred_config = ctx.dispatch_config.clone();
                    let binding_plan = vyre_driver::binding::BindingPlan::build(program)?;
                    let element_count =
                        vyre_driver::program_walks::dispatch_element_count(&binding_plan.bindings);
                    inferred_config.grid_override =
                        Some(vyre_driver::program_walks::infer_dispatch_grid_for_count(
                            element_count,
                            inferred_config
                                .workgroup_override
                                .unwrap_or(program.workgroup_size()),
                        )?);
                    &inferred_config
                } else {
                    &ctx.dispatch_config
                };
                vyre_driver::pipeline::compile_with_telemetry(
                    Arc::clone(&ctx.preferred_backend),
                    program,
                    compile_config,
                )
            })();
            match compile_res {
                Ok(build) => {
                    ctx.compiled_pipeline = Some(build.pipeline);
                    compile_cache_hit = build.cache_hit;
                }
                Err(error) => {
                    failed += 1;
                    cases_report.push(case_failure(
                        case,
                        Some(ctx.preferred_backend.id().to_string()),
                        format!("Compile error: {error}"),
                        case.performance_contract(),
                    ));
                    continue;
                }
            }
        }

        if let Some(hit) = compile_cache_hit {
            total_cache_observed += 1;
            if hit {
                total_cache_hits += 1;
            }
        }

        match run_case(case, &mut ctx, &mut prepared, suite, config) {
            Ok(case_report) => {
                if case_report.passes_summary_evidence() {
                    passed += 1;
                } else {
                    failed += 1;
                }
                cases_report.push(case_report);
            }
            Err(error) => {
                failed += 1;
                cases_report.push(case_failure(
                    case,
                    Some(ctx.preferred_backend.id().to_string()),
                    error,
                    case.performance_contract(),
                ));
            }
        }
    }

    let cache_hit_rate = if total_cache_observed > 0 {
        Some(total_cache_hits as f64 / total_cache_observed as f64)
    } else {
        None
    };

    let mut features = Vec::new();
    if let Some(backend) = config.backend_id.as_deref() {
        features.push(format!("backend:{backend}"));
    }
    if let Some(workgroup) = config.workgroup_override {
        features.push(format!(
            "workgroup:{}x{}x{}",
            workgroup[0], workgroup[1], workgroup[2]
        ));
    }

    let selected_backend = config.backend_id.clone().or_else(|| {
        cases_report
            .iter()
            .find_map(|case| case.backend_id.as_ref().cloned())
    });

    let source_provenance = vyre_driver::SourceProvenance::capture_current();
    let git = source_provenance.git.clone();
    let source_fingerprint = source_provenance.source_fingerprint;
    let source_tree_fingerprint = source_provenance.source_tree_fingerprint;
    let blockers = cases_report
        .iter()
        .flat_map(CaseReport::evidence_blockers)
        .collect();
    let report = ReportSchema {
        schema: "vyre-bench.result.v1".to_string(),
        run_id: format!("vyre-bench.{}", suite.as_str()),
        suite: suite.as_str().to_string(),
        selected_backend,
        backend_profile: selected_backend_profile,
        git,
        source_fingerprint,
        source_tree_fingerprint,
        environment,
        features,
        cases: cases_report,
        summary: ReportSummary {
            total_cases: selected_cases.len(),
            passed,
            failed,
            total_time_ns: started.elapsed().as_nanos() as u64,
            cache_hit_rate,
        },
        blockers,
    };

    if config.snapshot_on_pass && report.summary.failed == 0 {
        if let Err(error) = write_snapshot(&report) {
            eprintln!("Warning: failed to write benchmark snapshot: {error}");
        }
    }

    report
}

fn write_snapshot(report: &ReportSchema) -> Result<(), String> {
    let Some(commit) = report.git.get("commit") else {
        return Ok(());
    };
    let snapshot_dir = std::path::Path::new("snapshots");
    std::fs::create_dir_all(snapshot_dir).map_err(|error| error.to_string())?;
    let snapshot_path = snapshot_dir.join(format!("{commit}.json"));
    let file = std::fs::File::create(snapshot_path).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(file, report).map_err(|error| error.to_string())?;
    Ok(())
}

fn target_samples(suite: SuiteKind) -> usize {
    match suite {
        SuiteKind::Smoke => 10,
        SuiteKind::Release => 50,
        SuiteKind::Deep => 100,
        SuiteKind::Gpu
        | SuiteKind::Sweep
        | SuiteKind::CrossBackend
        | SuiteKind::Evolve
        | SuiteKind::Adversarial
        | SuiteKind::Competition
        | SuiteKind::Honest
        | SuiteKind::Custom(_) => 20,
    }
}

fn dispatch_config(config: &RunConfig) -> vyre::DispatchConfig {
    let mut dispatch = vyre::DispatchConfig::default();
    dispatch.workgroup_override = config.workgroup_override;
    dispatch
}

fn validate_requirements(
    environment: &EnvironmentData,
    requirements: &crate::api::case::BenchRequirements,
) -> Result<(), BenchError> {
    if requirements.needs_gpu && !environment.has_gpu {
        return Err(BenchError::GpuProbeFailed(format!(
            "has_gpu=false; probed_devices={}; features={}",
            environment.gpu_devices.len(),
            environment.features.join(",")
        )));
    }
    if let Some(min_input_bytes) = requirements.min_input_bytes {
        if min_input_bytes == 0 {
            return Err(BenchError::EnvironmentInvalid(
                "min_input_bytes must be non-zero when declared".to_string(),
            ));
        }
    }
    Ok(())
}

fn acquire_backend(
    backend_id: Option<&str>,
) -> Result<std::sync::Arc<dyn vyre::VyreBackend>, BenchError> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    #[allow(clippy::type_complexity)]
    static CACHE: OnceLock<Mutex<HashMap<Option<String>, Arc<dyn vyre::VyreBackend>>>> =
        OnceLock::new();

    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|error| BenchError::ExecutionFailed(format!(
            "benchmark backend cache lock was poisoned: {error}. Fix: restart the benchmark process after the panic that poisoned shared backend state."
        )))?;

    let key = backend_id.map(String::from);
    if let Some(backend) = cache.get(&key) {
        return Ok(Arc::clone(backend));
    }

    let backend: Arc<dyn vyre::VyreBackend> = match backend_id {
        Some(id) => vyre_driver::backend::acquire(id)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?,
        None => vyre_driver::backend::acquire_preferred_dispatch_backend()
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?,
    }
    .into();

    cache.insert(key, Arc::clone(&backend));
    Ok(backend)
}

fn case_failure(
    case: &'static dyn crate::api::case::BenchCase,
    backend_id: Option<String>,
    reason: String,
    contract: Option<PerformanceContract>,
) -> CaseReport {
    let meta = case.metadata();
    let requirements = case.requirements();
    eprintln!("Case {} failed: {}", meta.id.0, reason);
    let case_id = meta.id.0;
    let workload_fingerprint = format!("bench-case:{case_id}");
    let device_signature = backend_id
        .as_ref()
        .map(|backend| format!("unprobed-backend:{backend}"));
    CaseReport {
        id: case_id.clone(),
        workload_fingerprint: workload_fingerprint.clone(),
        name: meta.name,
        owner_crate: meta.owner_crate,
        workload_class: format!("{:?}", meta.workload),
        tags: meta.tags,
        backend_id,
        device_signature,
        held_out_corpus_id: Some(benchmark_held_out_corpus_id(&workload_fingerprint)),
        needs_gpu: requirements.needs_gpu,
        min_vram_bytes: requirements.min_vram_bytes,
        min_input_bytes: requirements.min_input_bytes,
        required_features: requirements.feature_set,
        status: "failed".to_string(),
        wall_ns: None,
        correctness: Correctness::Invalid { reason },
        contract,
        performance: None,
        metrics: BTreeMap::new(),
        optimization_passes_applied: vec![],
        artifacts: vec![],
    }
}

pub fn evaluate_candidate_headless(
    registry: &BenchRegistry,
    case_id: &str,
    candidate: vyre::ir::Program,
    config: &RunConfig,
) -> Result<CaseReport, String> {
    let environment =
        capture_environment().map_err(|error| format!("Environment probe failed: {error}"))?;
    let case = registry
        .get(&crate::api::case::BenchId(case_id.to_string()))
        .ok_or_else(|| format!("Unknown benchmark case: {}", case_id))?;

    let requirements = case.requirements();
    validate_requirements(&environment, &requirements)
        .map_err(|e| format!("Environment error: {}", e))?;

    let preferred_backend: Arc<dyn vyre::VyreBackend> =
        acquire_backend(config.backend_id.as_deref())
            .map_err(|error| format!("Backend error: {}", error))?;

    let mut ctx = BenchContext {
        backends: vec![],
        preferred_backend,
        compiled_pipeline: None,
        compiled_program_fingerprint: None,
        reference: crate::api::case::CpuReference {},
        optimizer: crate::api::case::OptimizerPipeline {},
        scratch: crate::api::case::ScratchPool { buffer: vec![] },
        rng: rand::SeedableRng::seed_from_u64(42),
        dispatch_config: vyre::DispatchConfig::default(),
        evolve_candidate: Some(candidate),
        include_baseline_outputs: false,
    };

    let mut prepared = case
        .prepare(&mut ctx)
        .map_err(|error| format!("Prepare error: {}", error))?;

    run_case(case, &mut ctx, &mut prepared, SuiteKind::Evolve, config)
}
