//! Cold-process compiled-pipeline persistence regression.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::harness::{fp_contract, OpEntry};

const HELPER_FLAG: &str = "VYRE_PIPELINE_CACHE_HELPER_OUT";
const HELPER_CASE_ID: &str = "VYRE_PIPELINE_CACHE_HELPER_CASE";
const PIPELINE_CACHE_CASE: &str = "vyre-libs::nn::softmax";
const CACHE_HIT_LATENCY_BUDGET: Duration = Duration::from_millis(500);

fn compile_case(case: &OpEntry) -> (Duration, Vec<Vec<u8>>) {
    let inputs = (case
        .test_inputs
        .expect("Fix: persistence regression requires deterministic test inputs"))(
    );
    let input_case = inputs
        .first()
        .cloned()
        .expect("Fix: persistence regression fixture must define at least one input case");
    let backend =
        WgpuBackend::acquire().expect("Fix: pipeline-cache persistence test requires a live GPU");
    let program = (case.build)();
    let compile_start = Instant::now();
    let _compiled = backend
        .compile(&program)
        .expect("Fix: persistence regression program must compile on the wgpu backend");
    let compile_time = compile_start.elapsed();
    let output = backend
        .dispatch(&program, &input_case, &DispatchConfig::default())
        .expect("Fix: persistence regression program must dispatch on the wgpu backend");
    if let Some(expected_cases) = case.expected_output {
        let expected = expected_cases()
            .into_iter()
            .next()
            .expect("Fix: expected-output fixture must contain one case");
        let tolerance = fp_contract::effective_tolerance(case.id, &program);
        assert_outputs_within_tolerance(case.id, tolerance, &expected, &output);
    }
    (compile_time, output)
}

fn f32_to_ordered(bits: u32) -> u32 {
    if (bits & 0x8000_0000) != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

fn assert_outputs_within_tolerance(
    op_id: &str,
    tolerance: u32,
    expected: &[Vec<u8>],
    actual: &[Vec<u8>],
) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "Fix: persistence regression fixture for {op_id} changed output buffer count"
    );
    for (buffer_index, (expected_buffer, actual_buffer)) in
        expected.iter().zip(actual.iter()).enumerate()
    {
        assert_eq!(
            expected_buffer.len(),
            actual_buffer.len(),
            "Fix: persistence regression fixture for {op_id} changed output buffer #{buffer_index} length"
        );
        if tolerance == 0 {
            assert_eq!(
                actual_buffer, expected_buffer,
                "Fix: persistence regression fixture for {op_id} must preserve byte-identical oracle output"
            );
            continue;
        }
        assert_eq!(
            expected_buffer.len() % 4,
            0,
            "Fix: tolerance-based persistence oracle for {op_id} requires f32-aligned output"
        );
        for (lane, (expected_word, actual_word)) in expected_buffer
            .chunks_exact(4)
            .zip(actual_buffer.chunks_exact(4))
            .enumerate()
        {
            let expected_bits = u32::from_le_bytes(expected_word.try_into().expect("4-byte chunk"));
            let actual_bits = u32::from_le_bytes(actual_word.try_into().expect("4-byte chunk"));
            let diff = f32_to_ordered(expected_bits).abs_diff(f32_to_ordered(actual_bits));
            assert!(
                diff <= tolerance,
                "Fix: persistence regression fixture for {op_id} exceeded {tolerance} ULP on buffer #{buffer_index} lane {lane}: expected=0x{expected_bits:08x}, actual=0x{actual_bits:08x}, diff={diff}"
            );
        }
    }
}

fn any_pipeline_blob(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && any_pipeline_blob(&path) {
            return true;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".pipeline.bin"))
        {
            return true;
        }
    }
    false
}

fn read_helper_result(root: &Path) -> (Duration, Vec<Vec<u8>>) {
    let nanos = fs::read_to_string(root.join("compile_nanos.txt"))
        .expect("Fix: helper process must write compile_nanos.txt")
        .trim()
        .parse::<u64>()
        .expect("Fix: helper compile_nanos.txt must contain a u64 nanosecond count");
    let mut outputs = Vec::new();
    let mut index = 0usize;
    loop {
        let path = root.join(format!("output-{index}.bin"));
        if !path.exists() {
            break;
        }
        outputs.push(
            fs::read(&path).unwrap_or_else(|error| {
                panic!("Fix: failed to read `{}`: {error}", path.display())
            }),
        );
        index += 1;
    }
    assert!(
        !outputs.is_empty(),
        "Fix: helper process must emit at least one output buffer"
    );
    (Duration::from_nanos(nanos), outputs)
}

fn helper_case(case_id: &str) -> &'static OpEntry {
    vyre_libs::harness::all_entries()
        .find(|entry| entry.id == case_id)
        .unwrap_or_else(|| panic!("Fix: no harness fixture registered for `{case_id}`"))
}

fn run_helper(cache_root: &Path, output_root: &Path, case_id: &str) -> (Duration, Vec<Vec<u8>>) {
    fs::create_dir_all(output_root).unwrap_or_else(|error| {
        panic!("Fix: failed to create `{}`: {error}", output_root.display())
    });
    let mut child =
        Command::new(std::env::current_exe().expect("Fix: test binary path must exist"))
            .arg("--exact")
            .arg("compiled_pipeline_cache_helper_process")
            .arg("--nocapture")
            .env(HELPER_FLAG, output_root)
            .env(HELPER_CASE_ID, case_id)
            .env("VYRE_CACHE_DIR", cache_root)
            .env("VYRE_AOT_CACHE_DIR", cache_root.join("aot"))
            .env("VYRE_PIPELINE_CACHE", "on")
            .env("WGPU_BACKEND", "vulkan")
            .spawn()
            .expect("Fix: failed to launch helper test process");

    let timeout = Duration::from_secs(120);
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    panic!("Fix: helper test process did not complete within {timeout:?}");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("Fix: failed to poll helper status: {e}"),
        }
    };
    assert!(
        status.success(),
        "Fix: helper test process exited with status {status}"
    );
    read_helper_result(output_root)
}

#[test]
fn compiled_pipeline_cache_helper_process() {
    let Some(output_root) = std::env::var_os(HELPER_FLAG).map(PathBuf::from) else {
        return;
    };
    let case_id =
        std::env::var(HELPER_CASE_ID).expect("Fix: helper process must receive a harness case id");
    let (compile_time, output) = compile_case(helper_case(&case_id));
    fs::write(
        output_root.join("compile_nanos.txt"),
        compile_time.as_nanos().to_string(),
    )
    .unwrap_or_else(|error| panic!("Fix: failed to write helper timing: {error}"));
    for (index, bytes) in output.iter().enumerate() {
        let path = output_root.join(format!("output-{index}.bin"));
        fs::write(&path, bytes)
            .unwrap_or_else(|error| panic!("Fix: failed to write `{}`: {error}", path.display()));
    }
}

#[test]
fn compiled_pipeline_cache_persists_across_backend_reconstruction() {
    let temp = TempDir::new().expect("Fix: tempdir required for cache isolation");
    let cache_root = temp.path().join("cache-root");
    let first_result_root = temp.path().join("first-run");
    let second_result_root = temp.path().join("second-run");

    let (first_compile, first_output) =
        run_helper(&cache_root, &first_result_root, PIPELINE_CACHE_CASE);
    let pipeline_cache_dir = cache_root.join("pipeline");
    assert!(
        any_pipeline_blob(&pipeline_cache_dir),
        "Fix: first helper process must persist a compiled pipeline blob under `{}`",
        pipeline_cache_dir.display()
    );

    let (second_compile, second_output) =
        run_helper(&cache_root, &second_result_root, PIPELINE_CACHE_CASE);

    assert_eq!(
        first_output, second_output,
        "Fix: compiled pipeline cache hit must preserve byte-identical dispatch output"
    );
    assert!(
        second_compile < first_compile || second_compile <= CACHE_HIT_LATENCY_BUDGET,
        "Fix: cold-process pipeline cache hit must either improve over first compile or stay under the hot-path latency budget of {CACHE_HIT_LATENCY_BUDGET:?} (first={first_compile:?}, second={second_compile:?})"
    );
}
