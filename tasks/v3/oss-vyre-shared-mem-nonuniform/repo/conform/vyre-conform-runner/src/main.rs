//! `vyre-conform` CLI  -  runs conformance certs for registered ops.

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::Serialize;
use vyre::ir::OpId;
use vyre::VyreBackend;
use vyre_conform_runner::convergence_lens;
use vyre_conform_runner::dispatch_grid;
use vyre_conform_runner::fp_parity::{compare_output_buffers, BufferParity};
use vyre_conform_runner::witness_plan::{
    plan_witness_inputs_into, static_buffer_byte_len, WitnessInputPlan,
};
use vyre_driver::{
    backend::{backend_dispatches, registered_backends},
    registry::DialectRegistry,
};
use vyre_reference::value::Value;

#[cfg(feature = "gpu")]
use vyre_driver_cuda as _;
#[cfg(feature = "gpu")]
use vyre_driver_metal as _;
use vyre_driver_reference as _;
#[cfg(feature = "gpu")]
use vyre_driver_wgpu as _;
use vyre_intrinsics as _;
use vyre_libs as _;

const DEFAULT_CERTIFICATE_DIR: &str = ".internals/certs/";
const DEFAULT_CERTIFICATE_FILE: &str = "prove.json";

#[derive(Clone, Debug, Serialize)]
struct PairResult {
    #[serde(serialize_with = "serialize_op_id")]
    op_id: OpId,
    backend_id: String,
    passed: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay_capsule: Option<ReplayCapsule>,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayCapsule {
    schema_version: u32,
    op_id: String,
    backend_id: String,
    case_index: usize,
    replay_command: String,
    program_blake3: String,
    witness_input_blake3: String,
    reference_output_blake3: String,
    backend_output_blake3: String,
    witness_input_buffers_hex: Vec<String>,
    reference_output_buffers_hex: Vec<String>,
    backend_output_buffers_hex: Vec<String>,
    witness_input_count: usize,
    reference_output_count: usize,
    backend_output_count: usize,
    first_mismatch: ReplayMismatch,
    minimization: ReplayMinimization,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayMismatch {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_byte: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend_byte: Option<u8>,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayMinimization {
    strategy: &'static str,
    original_case_count: usize,
    retained_case_count: usize,
}

#[derive(Debug, Serialize)]
struct ProveArtifact {
    wire_format_version: u32,
    program_hash: String,
    backend_id: String,
    plan: ProofPlanSummary,
    signature: String,
    public_key: String,
    pairs: Vec<PairResult>,
}

#[derive(Debug, Serialize)]
struct MergedProveArtifact {
    wire_format_version: u32,
    program_hash: String,
    backend_id: String,
    plan: ProofPlanSummary,
    signature: String,
    public_key: String,
    pairs: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ProofPlanSummary {
    backend_count: usize,
    op_count: usize,
    pair_count: usize,
    witness_case_count: usize,
    catalog_hash: String,
    execution_hash: String,
    selection: ProofSelectionSummary,
}

#[derive(Debug, Serialize)]
struct ProofSelectionSummary {
    backend_filter: String,
    ops_filter: String,
    shard_index: Option<usize>,
    shard_count: Option<usize>,
    universe_backend_count: usize,
    universe_op_count: usize,
    selected_backend_count: usize,
    selected_op_count: usize,
}

#[derive(Debug, Serialize)]
struct ProofPlanArtifact {
    wire_format_version: u32,
    plan: ProofPlanSummary,
    backends: Vec<String>,
    ops: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct ShardSpec {
    index: usize,
    count: usize,
}

#[derive(Debug)]
struct ProofOptions {
    out: Option<String>,
    certificates_dir: Option<String>,
    backend_filter: String,
    ops_filter: String,
    shard: Option<ShardSpec>,
}

/// Per-case fixture bytes  -  one outer Vec per dispatch case, one
/// middle Vec per declared buffer, one inner Vec of raw byte content.
type FixtureCases = Vec<Vec<Vec<u8>>>;
/// Signature of the zero-argument closure an `OpEntry` ships as its
/// `test_inputs` / `expected_output` generator.
type FixtureFn = fn() -> FixtureCases;

fn serialize_op_id<S>(op_id: &OpId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(op_id.as_ref())
}

#[derive(Clone, Copy)]
struct UnifiedEntry {
    id: &'static str,
    build: fn() -> vyre::Program,
    test_inputs: Option<FixtureFn>,
    expected_output: Option<FixtureFn>,
}

struct PreparedEntry {
    id: &'static str,
    program: vyre::Program,
    dispatch_config: vyre::DispatchConfig,
    cases: FixtureCases,
    reference_cases: FixtureCases,
    input_plan: BackendDispatchPlan,
    convergence_max_iterations: Option<u32>,
}

fn main() {
    let mut args = std::env::args();
    let _binary = args.next();
    let subcommand = match args.next() {
        Some(arg) => arg,
        None => {
            print_usage();
            return;
        }
    };
    if subcommand == "-h" || subcommand == "--help" {
        print_usage();
        return;
    }
    if subcommand == "prove" {
        if let Err(error) = prove(args) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if subcommand == "plan" {
        if let Err(error) = emit_plan(args) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if subcommand == "merge" {
        if let Err(error) = merge_certificates(args) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if subcommand != "dispatch" {
        eprintln!(
            "unknown subcommand `{}`  -  supported subcommands: dispatch, merge, plan, prove.",
            subcommand
        );
        std::process::exit(2);
    }

    let mut backend_value = None::<String>;
    let mut ops_value = None::<String>;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--backend" => {
                backend_value = it.next();
            }
            "--ops" => {
                ops_value = it.next();
            }
            other => {
                eprintln!("unknown flag `{other}`");
                std::process::exit(2);
            }
        }
    }

    let backend = backend_value.as_deref().unwrap_or("auto");
    let ops = ops_value.as_deref().unwrap_or("all");
    match dispatch_pairs(backend, ops) {
        Ok(pairs) => {
            let failed = pairs.iter().any(|pair| !pair.passed);
            for pair in pairs {
                let json = match serde_json::to_string(&pair) {
                    Ok(json) => json,
                    Err(error) => {
                        eprintln!("failed to serialize dispatch result: {error}");
                        std::process::exit(1);
                    }
                };
                println!("{json}");
            }
            if failed {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("usage: vyre-conform dispatch --backend <backend-id|auto> --ops <all|<op_id>>");
    println!("       vyre-conform plan [--out <plan.json>] [--backend <all|backend-id>] [--ops <all|op_id>] [--shard <index>/<count>]");
    println!("       vyre-conform merge --out <merged.json> <prove-shard.json>...");
    println!(
        "       vyre-conform prove [--out <cert.json>] [--certificates <dir>] [--backend <all|backend-id>] [--ops <all|op_id>] [--shard <index>/<count>]  # default: {DEFAULT_CERTIFICATE_DIR}/{DEFAULT_CERTIFICATE_FILE}"
    );
}

fn dispatch_pairs(backend_id: &str, ops: &str) -> Result<Vec<PairResult>, String> {
    let entries = unified_entries();
    let mut pairs = Vec::with_capacity(entries.len());
    let backend_id = backend_id.to_string();

    let mut selected_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        if ops != "all" && entry.id != ops {
            continue;
        }
        selected_entries.push(entry);
    }

    if ops != "all" && selected_entries.is_empty() {
        return Err(format!(
            "unknown op `{ops}`. Fix: pass `--ops all` or one registered OpEntry id."
        ));
    }

    pairs.reserve(selected_entries.len());
    for entry in selected_entries {
        let prepared = match prepare_entry(entry) {
            Ok(prepared) => prepared,
            Err(error) => {
                pairs.push(PairResult {
                    op_id: entry.id.into(),
                    backend_id: backend_id.clone(),
                    passed: false,
                    message: error,
                    replay_capsule: None,
                });
                continue;
            }
        };
        let backend = match acquire_backend(&backend_id) {
            Ok(backend) => backend,
            Err(error) => {
                pairs.push(PairResult {
                    op_id: entry.id.into(),
                    backend_id: backend_id.clone(),
                    passed: false,
                    message: format!(
                        "backend acquisition failed before dispatch: {error}. Fix: isolate or reset the backend after the preceding failing op, then repair the op that poisoned device state."
                    ),
                    replay_capsule: None,
                });
                continue;
            }
        };
        pairs.push(compare_backend_against_reference(
            backend.as_ref(),
            &backend_id,
            &prepared,
        ));
    }

    if ops != "all" && pairs.is_empty() {
        return Err(format!(
            "unknown op `{ops}`. Fix: pass `--ops all` or one registered OpEntry id."
        ));
    }

    Ok(pairs)
}

fn acquire_backend(backend_id: &str) -> Result<Box<dyn VyreBackend>, String> {
    let requested = if backend_id == "auto" {
        registered_backends()
            .iter()
            .find(|registration| backend_dispatches(registration.id))
            .map(|registration| registration.id)
            .ok_or_else(|| {
                "no dispatch-capable backend is linked into this binary. Fix: link a concrete driver crate that submits BackendCapability { dispatches: true }.".to_string()
            })?
    } else {
        backend_id
    };
    let registration = registered_backends()
        .iter()
        .find(|registration| registration.id == requested)
        .ok_or_else(|| {
            format!("unknown backend `{requested}`. Fix: link a concrete driver crate that registers this backend id.")
        })?;

    registration
        .acquire()
        .map_err(|error| format!("failed to acquire backend `{requested}`. Fix: {error}"))
}

fn dispatch_capable_backends() -> Vec<&'static vyre::BackendRegistration> {
    force_link_backend_inventory();
    registered_backends()
        .iter()
        .copied()
        .filter(|backend| backend_dispatches(backend.id))
        .collect()
}

fn force_link_backend_inventory() {
    #[cfg(feature = "gpu")]
    {
        let metal_acquire: fn() -> Result<Box<dyn VyreBackend>, vyre_driver::backend::BackendError> =
            vyre_driver_metal::acquire;
        std::hint::black_box(metal_acquire);
    }
}

fn parse_proof_options(
    command: &str,
    args: impl IntoIterator<Item = String>,
) -> Result<ProofOptions, String> {
    let mut out = None;
    let mut certificates_dir = None::<String>;
    let mut backend_filter = std::env::var("VYRE_BACKEND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "all".to_string());
    let mut ops_filter = "all".to_string();
    let mut shard = None::<ShardSpec>;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                out = Some(next_option_value(&mut it, "--out")?);
            }
            "--certificates" if command == "prove" => {
                certificates_dir = Some(next_option_value(&mut it, "--certificates")?);
            }
            "--backend" => {
                backend_filter = next_option_value(&mut it, "--backend")?;
            }
            "--ops" => {
                ops_filter = next_option_value(&mut it, "--ops")?;
            }
            "--shard" => {
                let value = next_option_value(&mut it, "--shard")?;
                shard = Some(parse_shard_spec(&value)?);
            }
            other => {
                return Err(format!(
                    "unknown flag `{other}`. Fix: use `vyre-conform {command} --out <path> [--backend <all|backend-id>] [--ops <all|op_id>] [--shard <index>/<count>]`."
                ));
            }
        }
    }
    Ok(ProofOptions {
        out,
        certificates_dir,
        backend_filter,
        ops_filter,
        shard,
    })
}

fn next_option_value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing value for {flag}. Fix: pass a non-empty value."))
}

fn parse_shard_spec(value: &str) -> Result<ShardSpec, String> {
    let (index, count) = value.split_once('/').ok_or_else(|| {
        format!("invalid shard `{value}`. Fix: use zero-based `--shard <index>/<count>`, for example `--shard 0/8`.")
    })?;
    let index = index.parse::<usize>().map_err(|error| {
        format!("invalid shard index `{index}`: {error}. Fix: use a zero-based integer.")
    })?;
    let count = count.parse::<usize>().map_err(|error| {
        format!("invalid shard count `{count}`: {error}. Fix: use a positive integer.")
    })?;
    if count == 0 {
        return Err("invalid shard count `0`. Fix: shard count must be positive.".to_string());
    }
    if index >= count {
        return Err(format!(
            "invalid shard `{value}`. Fix: shard index must be less than shard count."
        ));
    }
    Ok(ShardSpec { index, count })
}

fn select_backends(
    all_backends: &[&'static vyre::BackendRegistration],
    filter: &str,
) -> Result<Vec<&'static vyre::BackendRegistration>, String> {
    if filter == "all" {
        return Ok(all_backends.to_vec());
    }
    let selected = all_backends
        .iter()
        .copied()
        .filter(|backend| backend.id == filter)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        let known = all_backends
            .iter()
            .map(|backend| backend.id)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unknown or non-dispatch backend `{filter}`. Fix: pass `--backend all` or one dispatch-capable backend id: {known}."
        ));
    }
    Ok(selected)
}

fn select_entries(
    all_entries: &[UnifiedEntry],
    ops_filter: &str,
    shard: Option<ShardSpec>,
) -> Result<Vec<UnifiedEntry>, String> {
    let mut selected = Vec::new();
    let mut matched_ops_filter = false;
    for (entry_index, entry) in all_entries.iter().copied().enumerate() {
        if ops_filter != "all" && entry.id != ops_filter {
            continue;
        }
        matched_ops_filter = true;
        if let Some(shard) = shard {
            if entry_index % shard.count != shard.index {
                continue;
            }
        }
        selected.push(entry);
    }
    if ops_filter != "all" && !matched_ops_filter {
        return Err(format!(
            "unknown op `{ops_filter}`. Fix: pass `--ops all` or one registered OpEntry id."
        ));
    }
    if selected.is_empty() {
        return Err(
            "proof selection matched zero ops. Fix: choose a shard that contains at least one registered op or remove `--shard`."
                .to_string(),
        );
    }
    Ok(selected)
}

fn is_reference_backend(backend_id: &str) -> bool {
    backend_id == "cpu-ref" || backend_id == "reference"
}

fn unified_entries() -> Vec<UnifiedEntry> {
    // CRITIQUE_CONFORM_2026-04-23 H1: previous version only chained
    // vyre_libs + vyre_intrinsics, silently omitting the entire
    // vyre_primitives catalog (bitset, reduce, label, predicate,
    // fixpoint, etc.). Both `vyre-conform dispatch --ops all` and
    // `vyre-conform prove` therefore skipped every primitive op
    // without warning, producing certificates that claimed full
    // coverage while leaving primitive semantics untested against the
    // backend. Match the breadth of parity_matrix.rs by chaining
    // primitives in too.
    let mut entries = vyre_libs::harness::all_entries()
        .map(|entry| UnifiedEntry {
            id: entry.id,
            build: entry.build,
            test_inputs: entry.test_inputs,
            expected_output: entry.expected_output,
        })
        .chain(
            vyre_intrinsics::harness::all_entries().map(|entry| UnifiedEntry {
                id: entry.id,
                build: entry.build,
                test_inputs: entry.test_inputs,
                expected_output: entry.expected_output,
            }),
        )
        .chain(
            vyre_primitives::harness::all_entries().map(|entry| UnifiedEntry {
                id: entry.id,
                build: entry.build,
                test_inputs: entry.test_inputs,
                expected_output: entry.expected_output,
            }),
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.id.cmp(right.id));
    entries
}

fn prepare_entry(entry: UnifiedEntry) -> Result<PreparedEntry, String> {
    let program = (entry.build)();
    let dispatch_config = dispatch_grid::config_for_program(&program)?;
    let cases = match entry.test_inputs {
        Some(test_inputs) => test_inputs(),
        None => synthesize_witness_cases(&program)?,
    };
    // CRITIQUE_CONFORM_2026-04-23 H4: `compare_backend_against_reference`
    // returned `passed: true` with message "0 witness case(s) matched"
    // when test_inputs() produced an empty vector  -  an op that registered
    // a witness-input function returning `vec![]` received a passing
    // certificate with zero coverage, defeating the entire witness
    // discipline. Reject up front with a named Fix: hint so the author
    // fixes the fixture.
    if cases.is_empty() {
        return Err("empty witness fixture. Fix: op has zero witness cases  -  empty fixtures are not coverage. Populate test_inputs() with at least one case before running `vyre-conform dispatch`.".to_string());
    }
    let expected_cases = entry
        .expected_output
        .map(|expected_output| expected_output());
    if let Some(expected_cases) = &expected_cases {
        if expected_cases.len() != cases.len() {
            return Err(format!(
                "expected_output case count {} does not match test_inputs case count {}. Fix: every witness case must have exactly one oracle case.",
                expected_cases.len(),
                cases.len()
            ));
        }
    }
    let input_plan = backend_dispatch_plan(&program)?;
    let convergence_max_iterations =
        vyre_libs::harness::convergence_contract(entry.id).map(|contract| contract.max_iterations);
    let reference_cases = prepare_reference_cases(
        entry.id,
        &program,
        &cases,
        &input_plan,
        expected_cases,
        convergence_max_iterations,
    )?;

    Ok(PreparedEntry {
        id: entry.id,
        program,
        dispatch_config,
        cases,
        reference_cases,
        input_plan,
        convergence_max_iterations,
    })
}

struct PreparedEntryBatch {
    entries: Vec<PreparedEntry>,
    pairs: Vec<PairResult>,
    any_failed: bool,
}

fn proof_worker_count(item_count: usize) -> usize {
    if item_count == 0 {
        return 0;
    }

    let requested = std::env::var("VYRE_CONFORM_PROOF_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|workers| *workers > 0);
    let detected = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .max(8);

    requested.unwrap_or(detected).min(item_count)
}

fn proof_timing_enabled() -> bool {
    std::env::var("VYRE_CONFORM_PROOF_TIMING")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn proof_millis(elapsed: std::time::Duration) -> u128 {
    elapsed.as_millis()
}

fn proof_pair_timing_threshold_ms() -> u128 {
    std::env::var("VYRE_CONFORM_PROOF_PAIR_TIMING_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(250)
}

fn proof_pair_start_timing_enabled() -> bool {
    std::env::var("VYRE_CONFORM_PROOF_PAIR_START")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn emit_pair_proof_start(backend_id: &str, op_id: &str) {
    if proof_timing_enabled() && proof_pair_start_timing_enabled() {
        eprintln!("vyre-conform proof pair start: backend={backend_id} op={op_id}");
    }
}

fn emit_pair_proof_timing(
    backend_id: &str,
    op_id: &str,
    passed: bool,
    elapsed: std::time::Duration,
) {
    if !proof_timing_enabled() {
        return;
    }
    let elapsed_ms = proof_millis(elapsed);
    if elapsed_ms >= proof_pair_timing_threshold_ms() {
        eprintln!(
            "vyre-conform proof pair timing: backend={backend_id} op={op_id} passed={passed} elapsed_ms={elapsed_ms}"
        );
    }
}

fn emit_backend_proof_timing(
    backend_id: &str,
    pair_count: usize,
    worker_count: usize,
    elapsed: std::time::Duration,
) {
    if proof_timing_enabled() {
        eprintln!(
            "vyre-conform proof backend timing: backend={backend_id} pairs={pair_count} workers={worker_count} elapsed_ms={}",
            proof_millis(elapsed)
        );
    }
}

struct ProofTimingReport<'a> {
    out: &'a str,
    backend_count: usize,
    selected_op_count: usize,
    prepared_op_count: usize,
    pair_count: usize,
    worker_count: usize,
    prepare_elapsed: std::time::Duration,
    backend_elapsed: std::time::Duration,
    signing_elapsed: std::time::Duration,
    total_elapsed: std::time::Duration,
}

fn emit_proof_timing(report: ProofTimingReport<'_>) {
    if proof_timing_enabled() {
        eprintln!(
            "vyre-conform proof timing: out={} backends={} selected_ops={} prepared_ops={} pairs={} workers={} prepare_ms={} backend_ms={} signing_ms={} total_ms={}",
            report.out,
            report.backend_count,
            report.selected_op_count,
            report.prepared_op_count,
            report.pair_count,
            report.worker_count,
            proof_millis(report.prepare_elapsed),
            proof_millis(report.backend_elapsed),
            proof_millis(report.signing_elapsed),
            proof_millis(report.total_elapsed),
        );
    }
}

fn prepare_entries_in_parallel(
    entries: Vec<UnifiedEntry>,
    backends: &[&'static vyre::BackendRegistration],
) -> PreparedEntryBatch {
    if entries.is_empty() {
        return PreparedEntryBatch {
            entries: Vec::new(),
            pairs: Vec::new(),
            any_failed: false,
        };
    }

    let worker_count = proof_worker_count(entries.len());
    let mut buckets = (0..worker_count).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, entry) in entries.into_iter().enumerate() {
        buckets[index % worker_count].push((index, entry));
    }

    let mut outcomes = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(buckets.len());
        for bucket in buckets {
            let ids = bucket
                .iter()
                .map(|(index, entry)| (*index, entry.id))
                .collect::<Vec<_>>();
            handles.push((
                ids,
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(index, entry)| {
                            let op_id = entry.id;
                            (index, op_id, prepare_entry(entry))
                        })
                        .collect::<Vec<_>>()
                }),
            ));
        }

        for (ids, handle) in handles {
            match handle.join() {
                Ok(mut worker_outcomes) => outcomes.append(&mut worker_outcomes),
                Err(payload) => {
                    let message = format!(
                        "proof preparation worker panicked: {}. Fix: witness preparation must return explicit fixture failures instead of unwinding.",
                        panic_message(payload)
                    );
                    outcomes.extend(
                        ids.into_iter()
                            .map(|(index, op_id)| (index, op_id, Err(message.clone()))),
                    );
                }
            }
        }
    });
    outcomes.sort_by_key(|(index, _, _)| *index);

    let mut prepared_entries = Vec::with_capacity(outcomes.len());
    let mut pairs = Vec::new();
    let mut any_failed = false;
    for (_, op_id, outcome) in outcomes {
        match outcome {
            Ok(prepared) => prepared_entries.push(prepared),
            Err(error) => {
                for backend in backends {
                    pairs.push(PairResult {
                        op_id: op_id.into(),
                        backend_id: backend.id.to_string(),
                        passed: false,
                        message: error.clone(),
                        replay_capsule: None,
                    });
                }
                any_failed = true;
            }
        }
    }

    PreparedEntryBatch {
        entries: prepared_entries,
        pairs,
        any_failed,
    }
}

fn prove_backends_in_parallel(
    backends: &[&'static vyre::BackendRegistration],
    prepared_entries: &[PreparedEntry],
) -> Vec<Vec<PairResult>> {
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(backends.len());
        for &backend in backends {
            handles.push((
                backend,
                scope.spawn(move || prove_one_backend(backend, prepared_entries)),
            ));
        }

        let mut results = Vec::with_capacity(handles.len());
        for (backend, handle) in handles {
            match handle.join() {
                Ok(pairs) => results.push(pairs),
                Err(payload) => {
                    let message = format!(
                        "backend `{}` proof worker panicked: {}. Fix: proof workers must return pair failures instead of unwinding.",
                        backend.id,
                        panic_message(payload)
                    );
                    results.push(
                        prepared_entries
                            .iter()
                            .map(|entry| PairResult {
                                op_id: entry.id.into(),
                                backend_id: backend.id.to_string(),
                                passed: false,
                                message: message.clone(),
                                replay_capsule: None,
                            })
                            .collect(),
                    );
                }
            }
        }
        results
    })
}

fn prove_one_backend(
    backend: &'static vyre::BackendRegistration,
    prepared_entries: &[PreparedEntry],
) -> Vec<PairResult> {
    let started = std::time::Instant::now();
    if prepared_entries.is_empty() {
        return Vec::new();
    }

    let instance = match backend.acquire() {
        Ok(instance) => instance,
        Err(error) => {
            let backend_id = backend.id.to_string();
            return prepared_entries
                .iter()
                .map(|entry| PairResult {
                            op_id: entry.id.into(),
                            backend_id: backend_id.clone(),
                            passed: false,
                            message: format!(
                                "backend `{}` unavailable: {error}. Fix: make the backend available before claiming parity.",
                                backend.id
                            ),
                            replay_capsule: None,
                        })
                        .collect();
        }
    };
    let instance = instance.as_ref();

    let worker_count = proof_worker_count(prepared_entries.len());
    let mut buckets = (0..worker_count).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, entry) in prepared_entries.iter().enumerate() {
        buckets[index % worker_count].push((index, entry));
    }

    let mut indexed_pairs = Vec::with_capacity(prepared_entries.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(buckets.len());
        for bucket in buckets {
            let ids = bucket
                .iter()
                .map(|(index, entry)| (*index, entry.id))
                .collect::<Vec<_>>();
            handles.push((
                ids,
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(index, entry)| {
                            emit_pair_proof_start(backend.id, entry.id);
                            let pair_started = std::time::Instant::now();
                            let pair =
                                compare_backend_against_reference(instance, &backend.id, entry);
                            emit_pair_proof_timing(
                                backend.id,
                                entry.id,
                                pair.passed,
                                pair_started.elapsed(),
                            );
                            (index, pair)
                        })
                        .collect::<Vec<_>>()
                }),
            ));
        }

        for (ids, handle) in handles {
            match handle.join() {
                Ok(mut worker_pairs) => indexed_pairs.append(&mut worker_pairs),
                Err(payload) => {
                    let message = format!(
                        "backend `{}` proof shard worker panicked: {}. Fix: proof workers must return pair failures instead of unwinding.",
                        backend.id,
                        panic_message(payload)
                    );
                    indexed_pairs.extend(ids.into_iter().map(|(index, op_id)| {
                        (
                            index,
                            PairResult {
                                op_id: op_id.into(),
                                backend_id: backend.id.to_string(),
                                passed: false,
                                message: message.clone(),
                                replay_capsule: None,
                            },
                        )
                    }));
                }
            }
        }
    });

    indexed_pairs.sort_by_key(|(index, _)| *index);
    let pairs = indexed_pairs
        .into_iter()
        .map(|(_, pair)| pair)
        .collect::<Vec<_>>();
    emit_backend_proof_timing(backend.id, pairs.len(), worker_count, started.elapsed());
    pairs
}

fn prepare_reference_cases(
    op_id: &str,
    program: &vyre::Program,
    cases: &FixtureCases,
    input_plan: &BackendDispatchPlan,
    expected_cases: Option<FixtureCases>,
    convergence_max_iterations: Option<u32>,
) -> Result<FixtureCases, String> {
    let mut reference_cases = Vec::with_capacity(cases.len());
    if let Some(max_iterations) = convergence_max_iterations {
        for (case_index, inputs) in cases.iter().enumerate() {
            let outputs = convergence_lens::run_cpu_fixpoint_to_convergence(
                program,
                inputs,
                max_iterations,
            )
            .map_err(|error| {
                format!(
                    "{op_id}: CPU reference fixpoint loop failed while preparing case {case_index}: {error}. Fix: repair the witness or CPU reference before running backend parity."
                )
            })?;
            reference_cases.push(outputs);
        }
        return Ok(reference_cases);
    }

    if let Some(expected_cases) = expected_cases {
        return Ok(expected_cases);
    }

    let mut reference_values = Vec::with_capacity(program.buffers().len());
    let mut planned_inputs: Vec<&[u8]> = Vec::with_capacity(input_plan.source_count());
    for (case_index, inputs) in cases.iter().enumerate() {
        backend_dispatch_inputs_with_plan_into(inputs, input_plan, &mut planned_inputs).map_err(
            |error| {
                format!(
                    "{op_id}: reference input planning failed while preparing case {case_index}: {error}"
                )
            },
        )?;
        reference_values.clear();
        for input in &planned_inputs {
            reference_values.push(Value::from(*input));
        }
        let outputs = vyre_reference::reference_eval(program, &reference_values)
            .map_err(|error| {
                format!(
                    "{op_id}: reference dispatch failed while preparing case {case_index}: {error}. Fix: repair the witness or CPU reference before running backend parity."
                )
            })?
            .into_iter()
            .map(|value| value.to_bytes())
            .collect::<Vec<_>>();
        reference_cases.push(outputs);
    }
    Ok(reference_cases)
}

fn compare_backend_against_reference(
    backend: &dyn VyreBackend,
    backend_id: &str,
    prepared: &PreparedEntry,
) -> PairResult {
    let backend_id = backend_id.to_string();
    let mut checked_cases = 0usize;
    let mut backend_inputs: Vec<&[u8]> = Vec::with_capacity(prepared.input_plan.source_count());

    for (case_index, inputs) in prepared.cases.iter().enumerate() {
        let reference = &prepared.reference_cases[case_index];
        if let Some(max_iterations) = prepared.convergence_max_iterations {
            let outputs = match convergence_lens::run_fixpoint_to_convergence(
                backend,
                &prepared.program,
                inputs,
                max_iterations,
            ) {
                Ok(outputs) => outputs,
                Err(error) => {
                    return PairResult {
                        op_id: prepared.id.into(),
                        backend_id: backend_id.clone(),
                        passed: false,
                        message: format!(
                            "backend fixpoint loop failed on case {case_index}: {error}. Fix: align backend.dispatch with vyre-reference under the convergence lens."
                        ),
                        replay_capsule: None,
                    };
                }
            };

            if let BufferParity::Mismatch(detail) =
                compare_output_buffers(&prepared.program, &outputs, reference)
            {
                return PairResult {
                    op_id: prepared.id.into(),
                    backend_id: backend_id.clone(),
                    passed: false,
                    message: format!(
                        "backend output diverged from vyre-reference after fixpoint convergence on case {case_index}: {detail}. Fix: align backend.dispatch with vyre-reference under the backend-transcendental-aware ULP window (byte-exact for non-F32, <= program-derived ULP cap for F32)."
                    ),
                    replay_capsule: Some(build_replay_capsule(
                        &backend_id,
                        prepared,
                        case_index,
                        inputs,
                        &outputs,
                        reference,
                    )),
                };
            }
        } else {
            if let Err(error) = backend_dispatch_inputs_with_plan_into(
                inputs,
                &prepared.input_plan,
                &mut backend_inputs,
            ) {
                return PairResult {
                    op_id: prepared.id.into(),
                    backend_id: backend_id.clone(),
                    passed: false,
                    message: error,
                    replay_capsule: None,
                };
            }
            let dispatch_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch_for_conformance(
                    backend,
                    &backend_id,
                    &prepared.program,
                    &backend_inputs,
                    &prepared.dispatch_config,
                )
            }));
            match dispatch_result {
                Ok(Ok(outputs)) => {
                    if let BufferParity::Mismatch(detail) =
                        compare_output_buffers(&prepared.program, &outputs, reference)
                    {
                        return PairResult {
                            op_id: prepared.id.into(),
                            backend_id: backend_id.clone(),
                            passed: false,
                            message: format!(
                                "backend output diverged from vyre-reference on case {case_index}: {detail}. Fix: align backend.dispatch with vyre-reference under the backend-transcendental-aware ULP window (byte-exact for non-F32, <= program-derived ULP cap for F32)."
                            ),
                            replay_capsule: Some(build_replay_capsule(
                                &backend_id,
                                prepared,
                                case_index,
                                inputs,
                                &outputs,
                                reference,
                            )),
                        };
                    }
                }
                Ok(Err(error)) => {
                    return PairResult {
                        op_id: prepared.id.into(),
                        backend_id: backend_id.clone(),
                        passed: false,
                        message: format!(
                            "backend dispatch failed on case {case_index}: {error}. Fix: make backend.dispatch execute this witness."
                        ),
                        replay_capsule: None,
                    };
                }
                Err(payload) => {
                    return PairResult {
                        op_id: prepared.id.into(),
                        backend_id: backend_id.clone(),
                        passed: false,
                        message: format!(
                            "backend dispatch panicked on case {case_index}: {}. Fix: backend.dispatch must return BackendError instead of unwinding, then execute this witness.",
                            panic_message(payload)
                        ),
                        replay_capsule: None,
                    };
                }
            }
        }
        checked_cases += 1;
    }

    PairResult {
        op_id: prepared.id.into(),
        backend_id,
        passed: true,
        message: format!(
            "{checked_cases} witness case(s) matched vyre-reference byte-for-byte via backend.dispatch"
        ),
        replay_capsule: None,
    }
}

fn build_replay_capsule(
    backend_id: &str,
    prepared: &PreparedEntry,
    case_index: usize,
    inputs: &[Vec<u8>],
    backend_outputs: &[Vec<u8>],
    reference_outputs: &[Vec<u8>],
) -> ReplayCapsule {
    ReplayCapsule {
        schema_version: 1,
        op_id: prepared.id.to_string(),
        backend_id: backend_id.to_string(),
        case_index,
        replay_command: format!(
            "vyre-conform dispatch --backend {} --ops {}",
            backend_id, prepared.id
        ),
        program_blake3: hex::encode(prepared.program.content_hash()),
        witness_input_blake3: hash_buffer_stream(
            b"vyre-conform-runner/replay-capsule/witness-input/v1",
            inputs,
        ),
        reference_output_blake3: hash_buffer_stream(
            b"vyre-conform-runner/replay-capsule/reference-output/v1",
            reference_outputs,
        ),
        backend_output_blake3: hash_buffer_stream(
            b"vyre-conform-runner/replay-capsule/backend-output/v1",
            backend_outputs,
        ),
        witness_input_buffers_hex: hex_buffers(inputs),
        reference_output_buffers_hex: hex_buffers(reference_outputs),
        backend_output_buffers_hex: hex_buffers(backend_outputs),
        witness_input_count: inputs.len(),
        reference_output_count: reference_outputs.len(),
        backend_output_count: backend_outputs.len(),
        first_mismatch: first_replay_mismatch(backend_outputs, reference_outputs),
        minimization: ReplayMinimization {
            strategy: "single_witness_case",
            original_case_count: prepared.cases.len(),
            retained_case_count: 1,
        },
    }
}

fn hash_buffer_stream(domain: &[u8], buffers: &[Vec<u8>]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    update_hash_len(&mut hasher, buffers.len());
    for buffer in buffers {
        update_hash_len(&mut hasher, buffer.len());
        hasher.update(buffer);
    }
    hasher.finalize().to_hex().to_string()
}

fn update_hash_len(hasher: &mut blake3::Hasher, len: usize) {
    let encoded = u64::try_from(len).unwrap_or(u64::MAX).to_le_bytes();
    hasher.update(&encoded);
}

fn hex_buffers(buffers: &[Vec<u8>]) -> Vec<String> {
    buffers.iter().map(hex::encode).collect()
}

fn first_replay_mismatch(
    backend_outputs: &[Vec<u8>],
    reference_outputs: &[Vec<u8>],
) -> ReplayMismatch {
    if backend_outputs.len() != reference_outputs.len() {
        return ReplayMismatch {
            kind: "output_count",
            output_index: None,
            byte_index: None,
            reference_len: Some(reference_outputs.len()),
            backend_len: Some(backend_outputs.len()),
            reference_byte: None,
            backend_byte: None,
        };
    }

    for (output_index, (backend, reference)) in backend_outputs
        .iter()
        .zip(reference_outputs.iter())
        .enumerate()
    {
        if backend.len() != reference.len() {
            return ReplayMismatch {
                kind: "output_length",
                output_index: Some(output_index),
                byte_index: None,
                reference_len: Some(reference.len()),
                backend_len: Some(backend.len()),
                reference_byte: None,
                backend_byte: None,
            };
        }
        if let Some((byte_index, (backend_byte, reference_byte))) = backend
            .iter()
            .copied()
            .zip(reference.iter().copied())
            .enumerate()
            .find(|(_, (backend_byte, reference_byte))| backend_byte != reference_byte)
        {
            return ReplayMismatch {
                kind: "byte",
                output_index: Some(output_index),
                byte_index: Some(byte_index),
                reference_len: Some(reference.len()),
                backend_len: Some(backend.len()),
                reference_byte: Some(reference_byte),
                backend_byte: Some(backend_byte),
            };
        }
    }

    ReplayMismatch {
        kind: "unclassified",
        output_index: None,
        byte_index: None,
        reference_len: None,
        backend_len: None,
        reference_byte: None,
        backend_byte: None,
    }
}

type BackendDispatchPlan = WitnessInputPlan;

fn backend_dispatch_plan(program: &vyre::Program) -> Result<BackendDispatchPlan, String> {
    WitnessInputPlan::for_program(program)
}

fn backend_dispatch_inputs_with_plan_into<'a>(
    fixture_inputs: &'a [Vec<u8>],
    plan: &'a BackendDispatchPlan,
    backend_inputs: &mut Vec<&'a [u8]>,
) -> Result<(), String> {
    plan_witness_inputs_into(fixture_inputs, plan, backend_inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

    #[test]
    fn backend_dispatch_plan_accepts_fixture_backed_runtime_sized_read_input() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let plan = backend_dispatch_plan(&program)
            .expect("Fix: runtime-sized read-only buffers must be fixture-backed, not rejected.");
        let case = vec![vec![0xA5; 12]];
        let mut backend_inputs = Vec::new();

        backend_dispatch_inputs_with_plan_into(&case, &plan, &mut backend_inputs)
            .expect("Fix: concrete fixture bytes must satisfy a runtime-sized input buffer.");

        assert_eq!(
            backend_inputs,
            vec![case[0].as_slice()],
            "Fix: dynamic fixture-backed inputs must be passed through byte-exactly."
        );
    }

    #[test]
    fn backend_dispatch_plan_rejects_omitted_runtime_sized_read_write_input() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "scratch",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let plan = backend_dispatch_plan(&program)
            .expect("Fix: dynamic read-write buffers may be fixture-backed per case.");
        let mut backend_inputs = Vec::new();

        let error = backend_dispatch_inputs_with_plan_into(&[], &plan, &mut backend_inputs)
            .expect_err("Fix: omitted dynamic read-write input must not be silently zeroed.");

        assert!(
            error.contains("runtime-sized read-write buffer"),
            "Fix: error must explain that dynamic read-write buffers need concrete fixture bytes, got: {error}"
        );
    }

    #[test]
    fn prepare_reference_cases_uses_planned_zeroed_read_write_inputs() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::storage("scratch", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "scratch",
                vyre::ir::Expr::u32(0),
                vyre::ir::Expr::load("input", vyre::ir::Expr::u32(0)),
            )],
        );
        let input_plan = backend_dispatch_plan(&program)
            .expect("Fix: static read-write zero-fill planning must succeed.");
        let cases = vec![vec![1u32.to_le_bytes().to_vec()]];

        let reference_cases = prepare_reference_cases(
            "test.planned_reference",
            &program,
            &cases,
            &input_plan,
            None,
            None,
        )
        .expect("Fix: reference preparation must use planned zeroed read-write inputs.");

        assert_eq!(
            reference_cases,
            vec![vec![1u32.to_le_bytes().to_vec()]],
            "Fix: prove reference preparation must match the input stream used by backend dispatch."
        );
    }

    #[test]
    fn replay_capsule_records_hashes_and_first_byte_mismatch() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let input = 7u32.to_le_bytes().to_vec();
        let reference_output = vec![1, 2, 3, 4];
        let backend_output = vec![1, 2, 9, 4];
        let prepared = PreparedEntry {
            id: "test.replay_capsule",
            dispatch_config: dispatch_grid::config_for_program(&program)
                .expect("Fix: one-workgroup program must have a dispatch grid."),
            input_plan: backend_dispatch_plan(&program)
                .expect("Fix: replay-capsule test program must plan backend inputs."),
            program,
            cases: vec![vec![input.clone()]],
            reference_cases: vec![vec![reference_output.clone()]],
            convergence_max_iterations: None,
        };

        let capsule = build_replay_capsule(
            "metal",
            &prepared,
            0,
            &prepared.cases[0],
            &[backend_output],
            &[reference_output],
        );

        assert_eq!(capsule.schema_version, 1);
        assert_eq!(capsule.op_id, "test.replay_capsule");
        assert_eq!(capsule.backend_id, "metal");
        assert_eq!(capsule.case_index, 0);
        assert_eq!(
            capsule.replay_command,
            "vyre-conform dispatch --backend metal --ops test.replay_capsule"
        );
        assert_hex64(&capsule.program_blake3);
        assert_hex64(&capsule.witness_input_blake3);
        assert_hex64(&capsule.reference_output_blake3);
        assert_hex64(&capsule.backend_output_blake3);
        assert_eq!(capsule.witness_input_buffers_hex, vec![hex::encode(input)]);
        assert_eq!(capsule.reference_output_buffers_hex, vec!["01020304"]);
        assert_eq!(capsule.backend_output_buffers_hex, vec!["01020904"]);
        assert_eq!(capsule.witness_input_count, 1);
        assert_eq!(capsule.reference_output_count, 1);
        assert_eq!(capsule.backend_output_count, 1);
        assert_eq!(capsule.first_mismatch.kind, "byte");
        assert_eq!(capsule.first_mismatch.output_index, Some(0));
        assert_eq!(capsule.first_mismatch.byte_index, Some(2));
        assert_eq!(capsule.first_mismatch.reference_byte, Some(3));
        assert_eq!(capsule.first_mismatch.backend_byte, Some(9));
        assert_eq!(capsule.minimization.strategy, "single_witness_case");
        assert_eq!(capsule.minimization.original_case_count, 1);
        assert_eq!(capsule.minimization.retained_case_count, 1);
    }

    #[test]
    fn pair_result_omits_capsule_on_success_and_serializes_capsule_on_failure() {
        let success = PairResult {
            op_id: "test.success".into(),
            backend_id: "metal".to_string(),
            passed: true,
            message: "ok".to_string(),
            replay_capsule: None,
        };
        let success_json =
            serde_json::to_value(&success).expect("Fix: success pair must serialize.");
        assert!(
            success_json.get("replay_capsule").is_none(),
            "Fix: passing pairs must not grow empty replay_capsule fields."
        );

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let prepared = PreparedEntry {
            id: "test.failure",
            dispatch_config: dispatch_grid::config_for_program(&program)
                .expect("Fix: output-only program must have a dispatch grid."),
            input_plan: backend_dispatch_plan(&program)
                .expect("Fix: output-only program must still have a backend plan."),
            program,
            cases: vec![vec![]],
            reference_cases: vec![vec![vec![0]]],
            convergence_max_iterations: None,
        };
        let failure = PairResult {
            op_id: "test.failure".into(),
            backend_id: "metal".to_string(),
            passed: false,
            message: "diverged".to_string(),
            replay_capsule: Some(build_replay_capsule(
                "metal",
                &prepared,
                0,
                &prepared.cases[0],
                &[vec![1]],
                &[vec![0]],
            )),
        };
        let failure_json =
            serde_json::to_value(&failure).expect("Fix: failure pair must serialize.");
        let capsule = failure_json
            .get("replay_capsule")
            .expect("Fix: diverging pairs must serialize replay_capsule.");
        assert_eq!(capsule["first_mismatch"]["kind"], "byte");
        assert_eq!(capsule["first_mismatch"]["byte_index"], 0);
        assert_eq!(capsule["backend_output_buffers_hex"][0], "01");
        assert_eq!(capsule["reference_output_buffers_hex"][0], "00");
    }

    fn assert_hex64(value: &str) {
        assert_eq!(value.len(), 64, "Fix: replay fingerprints must be BLAKE3 hex.");
        assert!(
            value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Fix: replay fingerprints must contain only hex characters."
        );
    }
}

fn synthesize_witness_cases(program: &vyre::Program) -> Result<FixtureCases, String> {
    let mut case = Vec::new();
    for buffer in program.buffers() {
        if buffer.kind() == vyre::ir::MemoryKind::Shared
            || buffer.is_output()
            || (buffer.is_pipeline_live_out()
                && matches!(buffer.access(), vyre::ir::BufferAccess::ReadWrite))
        {
            continue;
        }
        let byte_len = static_buffer_byte_len(buffer, "synthetic witness buffer")?;
        if byte_len == 0 {
            return Err(format!(
                "missing test_inputs for dynamically sized buffer `{}`. Fix: provide explicit witness bytes because synthetic conformance cannot infer runtime length.",
                buffer.name()
            ));
        }
        case.push(synthetic_buffer_bytes(&buffer.element(), byte_len));
    }
    if case.is_empty() {
        return Err(
            "missing test_inputs and Program has no synthesizable input buffers. Fix: provide explicit witness bytes for this op."
                .to_string(),
        );
    }
    Ok(vec![case])
}

fn synthetic_buffer_bytes(element: &vyre::ir::DataType, byte_len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; byte_len];
    match element {
        vyre::ir::DataType::F32 => {
            for chunk in bytes.chunks_exact_mut(4) {
                chunk.copy_from_slice(&1.0f32.to_le_bytes());
            }
        }
        vyre::ir::DataType::F64 => {
            for chunk in bytes.chunks_exact_mut(8) {
                chunk.copy_from_slice(&1.0f64.to_le_bytes());
            }
        }
        vyre::ir::DataType::F16 | vyre::ir::DataType::BF16 => {
            for chunk in bytes.chunks_exact_mut(2) {
                chunk.copy_from_slice(&0x3c00u16.to_le_bytes());
            }
        }
        _ => {}
    }
    bytes
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn dispatch_for_conformance(
    backend: &dyn VyreBackend,
    backend_id: &str,
    program: &vyre::Program,
    inputs: &[&[u8]],
    config: &vyre::DispatchConfig,
) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
    if backend_id == "cuda"
        && vyre_driver::grid_sync::contains_grid_sync(program)
        && !backend.supports_grid_sync()
    {
        return vyre_driver::grid_sync::dispatch_with_grid_sync_split(
            backend, program, inputs, config,
        );
    }
    backend.dispatch_borrowed(program, inputs, config)
}

fn emit_plan(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let options = parse_proof_options("plan", args)?;
    let _reg = DialectRegistry::global();
    let all_backends = dispatch_capable_backends();
    if all_backends.is_empty() {
        return Err(
            "plan refused to emit: no dispatch-capable backend is linked into this binary. \
             Fix: build with `--features gpu` or link another real dispatch backend."
                .to_string(),
        );
    }
    let backends = select_backends(&all_backends, &options.backend_filter)?;
    let all_entries = unified_entries();
    let entries = select_entries(&all_entries, &options.ops_filter, options.shard)?;
    let mut prepared_entries = Vec::with_capacity(entries.len());
    let mut failures = Vec::new();
    for entry in entries {
        match prepare_entry(entry) {
            Ok(prepared) => prepared_entries.push(prepared),
            Err(error) => failures.push(format!("{}: {}", entry.id, error)),
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "plan refused to emit because {} selected op(s) cannot produce executable witnesses:\n{}\nFix: repair every witness before planning conformance shards.",
            failures.len(),
            failures.join("\n")
        ));
    }
    let pair_count = backends.len().saturating_mul(prepared_entries.len());
    let plan = proof_plan_summary(
        &all_backends,
        &all_entries,
        &backends,
        &prepared_entries,
        pair_count,
        &options,
    );
    let artifact = ProofPlanArtifact {
        wire_format_version: 1,
        backends: backends
            .iter()
            .map(|backend| backend.id.to_string())
            .collect(),
        ops: prepared_entries
            .iter()
            .map(|entry| entry.id.to_string())
            .collect(),
        plan,
    };
    let json = serde_json::to_string_pretty(&artifact).map_err(|error| {
        format!("failed to serialize proof plan: {error}. Fix: keep plan fields JSON-serializable.")
    })?;
    if let Some(out) = options.out.as_deref() {
        write_json_artifact(out, json, "proof plan")
    } else {
        println!("{json}");
        Ok(())
    }
}

fn write_json_artifact(out: &str, json: String, artifact_kind: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(out).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create {artifact_kind} directory `{}`: {error}. Fix: choose a writable --out parent.",
                    parent.display()
                )
            })?;
        }
    }
    std::fs::write(out, json).map_err(|error| {
        format!(
            "failed to write {artifact_kind} `{out}`: {error}. Fix: choose a writable --out path."
        )
    })
}

struct VerifiedShard {
    path: String,
    value: serde_json::Value,
    catalog_hash: String,
    execution_hash: String,
    program_hash: String,
    witness_case_count: usize,
    pair_count: usize,
    universe_backend_count: usize,
    universe_op_count: usize,
}

fn merge_certificates(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut out = None::<String>;
    let mut paths = Vec::new();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => out = Some(next_option_value(&mut it, "--out")?),
            other => paths.push(other.to_string()),
        }
    }
    let out = out.ok_or_else(|| {
        "missing --out for merge. Fix: run `vyre-conform merge --out <merged.json> <prove-shard.json>...`."
            .to_string()
    })?;
    if paths.is_empty() {
        return Err(
            "merge refused to emit: no certificates were provided. Fix: pass one or more signed prove artifacts."
                .to_string(),
        );
    }

    let mut catalog_hash = None::<String>;
    let mut source_hashes = Vec::with_capacity(paths.len());
    let mut pair_map = BTreeMap::<(String, String), serde_json::Value>::new();
    let mut unique_backends = BTreeSet::<String>::new();
    let mut unique_ops = BTreeSet::<String>::new();
    let mut witness_case_count = 0usize;
    let mut universe_backend_count = 0usize;
    let mut universe_op_count = 0usize;
    let mut merge_hasher = blake3::Hasher::new();
    merge_hasher.update(b"vyre-conform-runner/proof-merge/v1");

    for path in paths {
        let shard = read_and_verify_shard(&path)?;
        match &catalog_hash {
            Some(expected) if expected != &shard.catalog_hash => {
                return Err(format!(
                    "merge refused `{path}`: catalog_hash `{}` differs from `{expected}`. Fix: only merge shards produced from the same executable registry.",
                    shard.catalog_hash
                ));
            }
            None => catalog_hash = Some(shard.catalog_hash.clone()),
            _ => {}
        }
        merge_hasher.update(shard.program_hash.as_bytes());
        merge_hasher.update(shard.execution_hash.as_bytes());
        source_hashes.push(shard.program_hash.clone());
        witness_case_count = witness_case_count.saturating_add(shard.witness_case_count);
        universe_backend_count = universe_backend_count.max(shard.universe_backend_count);
        universe_op_count = universe_op_count.max(shard.universe_op_count);

        let pairs = value_field(&shard.value, "pairs", &shard.path)?
            .as_array()
            .ok_or_else(|| {
                format!(
                    "certificate `{}` has non-array `pairs`. Fix: only merge prove artifacts.",
                    shard.path
                )
            })?;
        if pairs.len() != shard.pair_count {
            return Err(format!(
                "certificate `{}` plan pair_count={} but pairs.len()={}. Fix: regenerate the shard; the signed plan must match the body.",
                shard.path,
                shard.pair_count,
                pairs.len()
            ));
        }
        for pair in pairs {
            let backend = string_field(pair, "backend_id", &shard.path)?.to_string();
            let op = string_field(pair, "op_id", &shard.path)?.to_string();
            let passed = value_field(pair, "passed", &shard.path)?
                .as_bool()
                .ok_or_else(|| {
                    format!(
                        "certificate `{}` pair ({backend}, {op}) has non-boolean `passed`. Fix: regenerate the shard.",
                        shard.path
                    )
                })?;
            if !passed {
                return Err(format!(
                    "merge refused failing pair ({backend}, {op}) from `{}`. Fix: repair the backend/op divergence before merging.",
                    shard.path
                ));
            }
            let key = (backend.clone(), op.clone());
            unique_backends.insert(backend);
            unique_ops.insert(op);
            if pair_map.insert(key.clone(), pair.clone()).is_some() {
                return Err(format!(
                    "merge refused duplicate pair ({}, {}) from `{}`. Fix: merge disjoint shards or remove duplicate certificates.",
                    key.0, key.1, shard.path
                ));
            }
        }
    }

    let catalog_hash = catalog_hash.ok_or_else(|| {
        "merge refused to emit: no catalog hash was observed. Fix: pass valid prove artifacts."
            .to_string()
    })?;
    let pairs = pair_map.into_values().collect::<Vec<_>>();
    for pair in &pairs {
        merge_hasher.update(string_field(pair, "backend_id", "merged artifact")?.as_bytes());
        merge_hasher.update(string_field(pair, "op_id", "merged artifact")?.as_bytes());
        merge_hasher.update(string_field(pair, "message", "merged artifact")?.as_bytes());
    }
    for source_hash in &source_hashes {
        merge_hasher.update(source_hash.as_bytes());
    }
    let execution_hash = merge_hasher.finalize().to_hex().to_string();
    let plan = ProofPlanSummary {
        backend_count: unique_backends.len(),
        op_count: unique_ops.len(),
        pair_count: pairs.len(),
        witness_case_count,
        catalog_hash,
        execution_hash,
        selection: ProofSelectionSummary {
            backend_filter: "merged".to_string(),
            ops_filter: "merged".to_string(),
            shard_index: None,
            shard_count: Some(source_hashes.len()),
            universe_backend_count,
            universe_op_count,
            selected_backend_count: unique_backends.len(),
            selected_op_count: unique_ops.len(),
        },
    };

    let mut program_hasher = blake3::Hasher::new();
    program_hasher.update(b"vyre-conform-runner/merge/v1");
    hash_proof_plan(&mut program_hasher, &plan);
    for pair in &pairs {
        program_hasher.update(string_field(pair, "backend_id", "merged artifact")?.as_bytes());
        program_hasher.update(string_field(pair, "op_id", "merged artifact")?.as_bytes());
        program_hasher.update(string_field(pair, "message", "merged artifact")?.as_bytes());
    }
    let program_hash = program_hasher.finalize().to_hex().to_string();

    use rand_core::RngCore;
    let mut seed = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut seed);
    let key = SigningKey::from_bytes(&seed);
    let signable = serde_json::json!({
        "wire_format_version": 1u32,
        "program_hash": program_hash,
        "backend_id": "merged",
        "plan": &plan,
        "pairs": &pairs,
    });
    let signable_bytes = serde_json::to_vec(&signable).map_err(|error| {
        format!("failed to serialize merged prove artifact body: {error}. Fix: keep certificate fields JSON-serializable.")
    })?;
    let signature = key.sign(&signable_bytes);
    let artifact = MergedProveArtifact {
        wire_format_version: 1,
        program_hash,
        backend_id: "merged".to_string(),
        plan,
        signature: hex::encode(signature.to_bytes()),
        public_key: hex::encode(key.verifying_key().to_bytes()),
        pairs,
    };
    let json = serde_json::to_string_pretty(&artifact).map_err(|error| {
        format!("failed to serialize merged prove artifact: {error}. Fix: keep certificate fields JSON-serializable.")
    })?;
    write_json_artifact(&out, json, "merged prove artifact")
}

fn read_and_verify_shard(path: &str) -> Result<VerifiedShard, String> {
    let json = read_prove_artifact_bounded(path)?;
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|error| {
        format!(
            "failed to parse certificate `{path}`: {error}. Fix: pass a valid JSON prove artifact."
        )
    })?;
    let wire_format_version = u32_field(&value, "wire_format_version", path)?;
    if wire_format_version != 1 {
        return Err(format!(
            "certificate `{path}` has wire_format_version {wire_format_version}. Fix: merge only v1 prove artifacts."
        ));
    }
    let program_hash = string_field(&value, "program_hash", path)?.to_string();
    let signature_hex = string_field(&value, "signature", path)?;
    let public_key_hex = string_field(&value, "public_key", path)?;
    let signature_bytes = hex::decode(signature_hex).map_err(|error| {
        format!("certificate `{path}` signature is not hex: {error}. Fix: regenerate the shard.")
    })?;
    let public_key_bytes = hex::decode(public_key_hex).map_err(|error| {
        format!("certificate `{path}` public_key is not hex: {error}. Fix: regenerate the shard.")
    })?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|error| {
        format!("certificate `{path}` signature is invalid: {error}. Fix: regenerate the shard.")
    })?;
    let public_key_array: [u8; 32] = public_key_bytes.as_slice().try_into().map_err(|_| {
        format!(
            "certificate `{path}` public_key must decode to 32 bytes. Fix: regenerate the shard."
        )
    })?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_array).map_err(|error| {
        format!("certificate `{path}` public_key is invalid: {error}. Fix: regenerate the shard.")
    })?;
    let signable = serde_json::json!({
        "wire_format_version": value["wire_format_version"].clone(),
        "program_hash": value["program_hash"].clone(),
        "backend_id": value["backend_id"].clone(),
        "plan": value["plan"].clone(),
        "pairs": value["pairs"].clone(),
    });
    let signable_bytes = serde_json::to_vec(&signable).map_err(|error| {
        format!("failed to serialize certificate `{path}` signable body: {error}. Fix: regenerate the shard.")
    })?;
    verifying_key
        .verify(&signable_bytes, &signature)
        .map_err(|error| {
            format!("certificate `{path}` signature verification failed: {error}. Fix: discard the tampered shard and rerun prove.")
        })?;

    let plan = value_field(&value, "plan", path)?;
    let selection = value_field(plan, "selection", path)?;
    let catalog_hash = string_field(plan, "catalog_hash", path)?.to_string();
    let execution_hash = string_field(plan, "execution_hash", path)?.to_string();
    let witness_case_count = usize_field(plan, "witness_case_count", path)?;
    let pair_count = usize_field(plan, "pair_count", path)?;
    let universe_backend_count = usize_field(selection, "universe_backend_count", path)?;
    let universe_op_count = usize_field(selection, "universe_op_count", path)?;

    let pairs = value_field(&value, "pairs", path)?
        .as_array()
        .ok_or_else(|| format!("certificate `{path}` has non-array pairs. Fix: regenerate it."))?;
    if pairs.is_empty() {
        return Err(format!(
            "certificate `{path}` has no pairs. Fix: prove artifacts must contain executable parity pairs."
        ));
    }
    for pair in pairs {
        let backend = string_field(pair, "backend_id", path)?;
        let op = string_field(pair, "op_id", path)?;
        let passed = value_field(pair, "passed", path)?
            .as_bool()
            .ok_or_else(|| {
                format!(
                    "certificate `{path}` pair ({backend}, {op}) has non-boolean `passed`. Fix: regenerate the shard."
                )
            })?;
        if !passed {
            return Err(format!(
                "certificate `{path}` contains failing pair ({backend}, {op}). Fix: repair the divergence before merging."
            ));
        }
    }

    Ok(VerifiedShard {
        path: path.to_string(),
        value,
        catalog_hash,
        execution_hash,
        program_hash,
        witness_case_count,
        pair_count,
        universe_backend_count,
        universe_op_count,
    })
}

const MAX_PROVE_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

fn read_prove_artifact_bounded(path: &str) -> Result<String, String> {
    let mut reader = std::fs::File::open(path).map_err(|error| {
        format!(
            "failed to read certificate `{path}`: {error}. Fix: pass a readable prove artifact."
        )
    })?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut reader, &mut chunk).map_err(|error| {
            format!(
                "failed to read certificate `{path}`: {error}. Fix: pass a readable prove artifact."
            )
        })?;
        if read == 0 {
            return String::from_utf8(bytes).map_err(|error| {
                format!(
                    "certificate `{path}` is not UTF-8: {error}. Fix: pass a valid JSON prove artifact."
                )
            });
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_PROVE_ARTIFACT_BYTES {
            return Err(format!(
                "certificate `{path}` exceeds {MAX_PROVE_ARTIFACT_BYTES} byte merge cap. Fix: shard or trim the prove artifact before merging."
            ));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

fn value_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
    path: &str,
) -> Result<&'a serde_json::Value, String> {
    value
        .get(field)
        .ok_or_else(|| format!("certificate `{path}` missing `{field}`. Fix: regenerate it."))
}

fn string_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
    path: &str,
) -> Result<&'a str, String> {
    value_field(value, field, path)?.as_str().ok_or_else(|| {
        format!("certificate `{path}` field `{field}` must be a string. Fix: regenerate it.")
    })
}

fn u32_field(value: &serde_json::Value, field: &str, path: &str) -> Result<u32, String> {
    let raw = value_field(value, field, path)?.as_u64().ok_or_else(|| {
        format!(
            "certificate `{path}` field `{field}` must be an unsigned integer. Fix: regenerate it."
        )
    })?;
    u32::try_from(raw).map_err(|_| {
        format!("certificate `{path}` field `{field}` exceeds u32::MAX. Fix: regenerate it.")
    })
}

fn usize_field(value: &serde_json::Value, field: &str, path: &str) -> Result<usize, String> {
    let raw = value_field(value, field, path)?.as_u64().ok_or_else(|| {
        format!(
            "certificate `{path}` field `{field}` must be an unsigned integer. Fix: regenerate it."
        )
    })?;
    usize::try_from(raw).map_err(|_| {
        format!("certificate `{path}` field `{field}` exceeds usize::MAX. Fix: regenerate it.")
    })
}

fn prove(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let total_started = std::time::Instant::now();
    let options = parse_proof_options("prove", args)?;
    let out = options
        .out
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            std::path::Path::new(
                options
                    .certificates_dir
                    .as_deref()
                    .unwrap_or(DEFAULT_CERTIFICATE_DIR),
            )
            .join(DEFAULT_CERTIFICATE_FILE)
            .to_string_lossy()
            .into_owned()
        });

    let _reg = DialectRegistry::global();
    let all_backends = dispatch_capable_backends();
    if all_backends.is_empty() {
        return Err(
            "prove refused to emit the certificate: no dispatch-capable backend is linked into this binary. \
             Fix: build with `--features gpu` (or another backend feature) so a backend that implements \
             real dispatch registers itself via `inventory::submit!(BackendCapability { dispatches: true, .. })`. \
             Emission-only backends are filtered out because they cannot execute Programs \
             against vyre-reference."
                .to_string(),
        );
    }
    let backends = select_backends(&all_backends, &options.backend_filter)?;
    if !backends
        .iter()
        .any(|backend| !is_reference_backend(backend.id))
    {
        return Err(
            "prove refused to emit the certificate: the selected backend set only contains reference dispatch backends. \
             Fix: build with `--features gpu` so certificate generation proves at least one real GPU backend \
             against vyre-reference instead of certifying the reference executor against itself."
                .to_string(),
        );
    }
    let all_entries = unified_entries();
    let entries = select_entries(&all_entries, &options.ops_filter, options.shard)?;
    let selected_op_count = entries.len();
    let worker_count = proof_worker_count(selected_op_count);
    let prepare_started = std::time::Instant::now();
    let prepared = prepare_entries_in_parallel(entries, &backends);
    let prepare_elapsed = prepare_started.elapsed();
    let prepared_entries = prepared.entries;
    let mut pairs = prepared.pairs;
    let mut any_failed = prepared.any_failed;
    let backend_started = std::time::Instant::now();
    for backend_pairs in prove_backends_in_parallel(&backends, &prepared_entries) {
        for pair in backend_pairs {
            if !pair.passed {
                any_failed = true;
            }
            pairs.push(pair);
        }
    }
    let backend_elapsed = backend_started.elapsed();
    if any_failed {
        use std::fmt::Write;
        let mut failing_count = 0usize;
        let mut failing_detail = String::new();
        for pair in pairs.iter().filter(|pair| !pair.passed) {
            if !failing_detail.is_empty() {
                failing_detail.push('\n');
            }
            let _ = write!(
                &mut failing_detail,
                "  - ({}, {}): {}",
                pair.backend_id, pair.op_id, pair.message
            );
            failing_count += 1;
        }
        return Err(format!(
            "prove refused to emit `{out}` because {} (backend, op) pair(s) diverged from vyre-reference:\n{}\nFix: resolve every failing pair before re-running prove.",
            failing_count,
            failing_detail
        ));
    }

    let plan = proof_plan_summary(
        &all_backends,
        &all_entries,
        &backends,
        &prepared_entries,
        pairs.len(),
        &options,
    );

    let signing_started = std::time::Instant::now();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-conform-runner/prove/v1");
    hash_proof_plan(&mut hasher, &plan);
    for pair in &pairs {
        hasher.update(pair.op_id.as_bytes());
        hasher.update(pair.backend_id.as_bytes());
        hasher.update(&[u8::from(pair.passed)]);
        hasher.update(pair.message.as_bytes());
    }
    let program_hash = hasher.finalize().to_hex().to_string();

    // CRITIQUE_CONFORM_2026-04-23 C2 (CRITICAL): the prior derivation
    // hashed `program_hash:pid:SystemTime::now()` into the Ed25519
    // seed. All three inputs are attacker-guessable (program_hash is
    // public, pid is ~2^22, SystemTime has microsecond resolution)
    // so an attacker who knew approximate CI runtime could brute-force
    // the seed and forge signed artifacts. The signature was
    // security theater.
    //
    // Use OS randomness instead. This makes every cert non-reproducible
    // (a feature  -  two runs of `prove` MUST produce different keys)
    // and removes the brute-force attack surface entirely. If a user
    // later needs reproducibility, they can thread a high-entropy
    // secret through an env var + HKDF; the insecure derivation above
    // is never the right answer.
    use rand_core::RngCore;
    let mut seed = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut seed);
    let key = SigningKey::from_bytes(&seed);
    let signable = serde_json::json!({
        "wire_format_version": 1u32,
        "program_hash": program_hash,
        "backend_id": "all",
        "plan": &plan,
        "pairs": &pairs,
    });
    let signable_bytes = serde_json::to_vec(&signable).map_err(|error| {
        format!("failed to serialize prove artifact body: {error}. Fix: keep certificate fields JSON-serializable.")
    })?;
    let signature = key.sign(&signable_bytes);
    let emitted_pair_count = pairs.len();
    let artifact = ProveArtifact {
        wire_format_version: 1,
        program_hash,
        backend_id: "all".to_string(),
        plan,
        signature: hex::encode(signature.to_bytes()),
        public_key: hex::encode(key.verifying_key().to_bytes()),
        pairs,
    };
    let json = serde_json::to_string_pretty(&artifact).map_err(|error| {
        format!("failed to serialize prove artifact: {error}. Fix: keep certificate fields JSON-serializable.")
    })?;
    let signing_elapsed = signing_started.elapsed();
    let result = write_json_artifact(&out, json, "prove artifact");
    if result.is_ok() {
        emit_proof_timing(ProofTimingReport {
            out: &out,
            backend_count: backends.len(),
            selected_op_count,
            prepared_op_count: prepared_entries.len(),
            pair_count: emitted_pair_count,
            worker_count,
            prepare_elapsed,
            backend_elapsed,
            signing_elapsed,
            total_elapsed: total_started.elapsed(),
        });
    }
    result
}

fn proof_plan_summary(
    universe_backends: &[&'static vyre::BackendRegistration],
    universe_entries: &[UnifiedEntry],
    backends: &[&'static vyre::BackendRegistration],
    entries: &[PreparedEntry],
    pair_count: usize,
    options: &ProofOptions,
) -> ProofPlanSummary {
    let mut catalog_hasher = blake3::Hasher::new();
    catalog_hasher.update(b"vyre-conform-runner/proof-catalog/v2");
    for backend in universe_backends {
        catalog_hasher.update(backend.id.as_bytes());
    }
    for entry in universe_entries {
        catalog_hasher.update(entry.id.as_bytes());
    }

    let mut execution_hasher = blake3::Hasher::new();
    execution_hasher.update(b"vyre-conform-runner/proof-execution/v2");
    for backend in backends {
        execution_hasher.update(backend.id.as_bytes());
    }
    let mut witness_case_count = 0usize;
    for entry in entries {
        execution_hasher.update(entry.id.as_bytes());
        execution_hasher.update(&entry.cases.len().to_le_bytes());
        execution_hasher.update(&entry.program.buffers().len().to_le_bytes());
        execution_hasher.update(&entry.input_plan.source_count().to_le_bytes());
        execution_hasher.update(&entry.input_plan.zeroed_input_count().to_le_bytes());
        execution_hasher.update(&entry.reference_cases.len().to_le_bytes());
        witness_case_count += entry.cases.len().saturating_mul(backends.len());
    }
    let selection = ProofSelectionSummary {
        backend_filter: options.backend_filter.clone(),
        ops_filter: options.ops_filter.clone(),
        shard_index: options.shard.map(|shard| shard.index),
        shard_count: options.shard.map(|shard| shard.count),
        universe_backend_count: universe_backends.len(),
        universe_op_count: universe_entries.len(),
        selected_backend_count: backends.len(),
        selected_op_count: entries.len(),
    };
    ProofPlanSummary {
        backend_count: backends.len(),
        op_count: entries.len(),
        pair_count,
        witness_case_count,
        catalog_hash: catalog_hasher.finalize().to_hex().to_string(),
        execution_hash: execution_hasher.finalize().to_hex().to_string(),
        selection,
    }
}

fn hash_proof_plan(hasher: &mut blake3::Hasher, plan: &ProofPlanSummary) {
    hasher.update(plan.catalog_hash.as_bytes());
    hasher.update(plan.execution_hash.as_bytes());
    hasher.update(&plan.backend_count.to_le_bytes());
    hasher.update(&plan.op_count.to_le_bytes());
    hasher.update(&plan.pair_count.to_le_bytes());
    hasher.update(&plan.witness_case_count.to_le_bytes());
    hasher.update(plan.selection.backend_filter.as_bytes());
    hasher.update(plan.selection.ops_filter.as_bytes());
    hash_optional_usize(hasher, plan.selection.shard_index);
    hash_optional_usize(hasher, plan.selection.shard_count);
    hasher.update(&plan.selection.universe_backend_count.to_le_bytes());
    hasher.update(&plan.selection.universe_op_count.to_le_bytes());
    hasher.update(&plan.selection.selected_backend_count.to_le_bytes());
    hasher.update(&plan.selection.selected_op_count.to_le_bytes());
}

fn hash_optional_usize(hasher: &mut blake3::Hasher, value: Option<usize>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
}
