//! CUDA-owned AOT launcher source emission.

use std::path::PathBuf;

use crate::backend::staging_reserve::reserve_vec;
use vyre_driver::aot::{AotLauncherFiles, AotLauncherRequest, LauncherDependency};

const CUDA_FFI: &str = include_str!("../templates/cuda_ffi.rs.tmpl");
const NCCL_FFI: &str = include_str!("../templates/nccl_ffi.rs.tmpl");

pub(crate) fn emit_launcher(request: &AotLauncherRequest<'_>) -> Result<AotLauncherFiles, String> {
    let file_count = if request.include_collectives { 3 } else { 2 };
    let mut entries = Vec::new();
    reserve_vec(&mut entries, file_count, "AOT launcher file entry").map_err(|error| {
        format!(
            "CUDA AOT launcher file list could not reserve {file_count} entry slot(s): {error}. Fix: reduce generated launcher sidecar count or split launcher emission."
        )
    })?;
    entries.push((PathBuf::from("src/main.rs"), emit_main(request)));
    entries.push((PathBuf::from("src/cuda_ffi.rs"), CUDA_FFI.to_string()));
    if request.include_collectives {
        entries.push((PathBuf::from("src/nccl_ffi.rs"), NCCL_FFI.to_string()));
    }

    Ok(AotLauncherFiles::from_entries(
        vec![LauncherDependency {
            name: "libc",
            spec: "\"0.2\"",
        }],
        entries,
    ))
}

fn emit_main(request: &AotLauncherRequest<'_>) -> String {
    let nccl_use = if request.include_collectives {
        "mod nccl_ffi;\nuse nccl_ffi as nccl;"
    } else {
        ""
    };
    let nccl_init = if request.include_collectives {
        r#"let world_size = parse_env_i32("WORLD_SIZE", 1)?;
        if world_size <= 0 {
            return Err("WORLD_SIZE must be positive. Fix: set WORLD_SIZE to the distributed rank count, or unset it for single-rank launch.".into());
        }
        let rank = parse_env_i32("RANK", 0)?;
        if rank < 0 || rank >= world_size {
            return Err(format!("RANK={rank} is outside WORLD_SIZE={world_size}. Fix: set RANK to a zero-based rank less than WORLD_SIZE.").into());
        }
        let nccl_comm = if world_size > 1 {
            Some(nccl::init_world(rank, world_size)?)
        } else {
            None
        };"#
    } else {
        "let nccl_comm: Option<()> = None;"
    };
    let nccl_drop = if request.include_collectives {
        "if let Some(comm) = nccl_comm { nccl::destroy(comm)?; }"
    } else {
        "drop(nccl_comm);"
    };
    let dispatch_block = if request.include_ttt_loop {
        r#"run_eval_time_training_loop(kernel, &bundle, &device_ptrs, metrics_idx, &mut kernel_args, &launch_limits)?;"#
    } else {
        r#"launch_manifest_kernel(kernel, &bundle, &device_ptrs, &mut kernel_args, &launch_limits)?;
    if let Some(idx) = metrics_idx {
        if device_ptrs.get(idx).is_none() {
            return Err("metrics buffer index was not backed by a CUDA allocation. Fix: repair the AOT manifest buffer table.".into());
        }
    }"#
    };

    format!(
        r##"//! Auto-generated PTX launcher.
//!
//! Self-contained launcher. It reads `manifest.json`, `kernel.<ext>.lzma`,
//! and `weights.brotli`, allocates device buffers, and dispatches the embedded
//! PTX kernel through the CUDA driver API.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

mod artifact;
mod cuda_ffi;
use cuda_ffi as cuda;

{nccl_use}

fn main() -> ExitCode {{
    match run() {{
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {{
            eprintln!("launcher error: {{e}}");
            ExitCode::FAILURE
        }}
    }}
}}

fn parse_env_i32(name: &str, default: i32) -> Result<i32, Box<dyn std::error::Error>> {{
    let Some(raw) = std::env::var(name).ok() else {{
        return Ok(default);
    }};
    raw.parse::<i32>().map_err(|error| {{
        format!("{{name}}={{raw:?}} is invalid: {{error}}. Fix: set {{name}} to a valid integer or unset it for the launcher default.").into()
    }})
}}

fn parse_env_u32(name: &str, default: u32) -> Result<u32, Box<dyn std::error::Error>> {{
    let Some(raw) = std::env::var(name).ok() else {{
        return Ok(default);
    }};
    raw.parse::<u32>().map_err(|error| {{
        format!("{{name}}={{raw:?}} is invalid: {{error}}. Fix: set {{name}} to a non-negative integer or unset it for the launcher default.").into()
    }})
}}

fn parse_env_optional_f32(name: &str) -> Result<Option<f32>, Box<dyn std::error::Error>> {{
    let Some(raw) = std::env::var(name).ok() else {{
        return Ok(None);
    }};
    let value = raw.parse::<f32>().map_err(|error| {{
        format!("{{name}}={{raw:?}} is invalid: {{error}}. Fix: set {{name}} to a finite floating-point threshold or unset it.").to_string()
    }})?;
    if !value.is_finite() {{
        return Err(format!("{{name}}={{raw:?}} is not finite. Fix: use a finite loss threshold.").into());
    }}
    Ok(Some(value))
}}

fn reserve_vec_to_capacity<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    context: &str,
    item: &str,
) -> Result<(), String> {{
    if target_capacity <= vec.capacity() {{
        return Ok(());
    }}
    vec.try_reserve_exact(target_capacity - vec.len()).map_err(|error| {{
        format!(
            "{{context}} could not reserve {{target_capacity}} {{item}} slot(s): {{error}}. Fix: shard the AOT bundle or reduce manifest fanout before launch."
        )
    }})
}}

fn run() -> Result<(), Box<dyn std::error::Error>> {{
    let bundle_dir = if let Some(arg) = env::args().nth(1) {{
        PathBuf::from(arg)
    }} else {{
        let exe = match env::current_exe() {{
            Ok(path) => path,
            Err(error) => {{
                return Err(format!("failed to resolve current executable path: {{error}}. Fix: pass the AOT bundle directory explicitly as argv[1].").into());
            }}
        }};
        match exe.parent() {{
            Some(parent) => PathBuf::from(parent),
            None => {{
                return Err("current executable has no parent directory. Fix: pass the AOT bundle directory explicitly as argv[1].".into());
            }}
        }}
    }};

    let bundle = artifact::load_bundle(&bundle_dir)?;
    validate_manifest_for_launch(&bundle.manifest)?;

    cuda::cu_init()?;
    let device_ordinal = select_cuda_device_ordinal()?;
    let device = cuda::cu_device_get(device_ordinal)?;
    let launch_limits = cuda::cu_device_launch_limits(device)?;
    let ctx = cuda::cu_ctx_create(device)?;
    let _ctx_guard = ctx;

    let module = cuda::cu_module_load_data(&bundle.kernel_bytes)?;
    let kernel = cuda::cu_module_get_function(&module, &bundle.manifest.entry_point)?;

    let mut device_ptrs: Vec<cuda::DeviceAllocation> = Vec::new();
    reserve_vec_to_capacity(
        &mut device_ptrs,
        bundle.manifest.buffers.len(),
        "AOT device allocation table",
        "CUDA buffer",
    )?;
    for index in 0..bundle.manifest.buffers.len() {{
        let bytes = manifest_buffer_allocation_bytes(&bundle.manifest, index)?;
        let dptr = cuda::cu_mem_alloc(bytes)?;
        device_ptrs.push(dptr);
    }}

    if let Some(params_dptr) = device_ptrs.first() {{
        let weight_bytes = u64::try_from(bundle.weight_bytes.len())
            .map_err(|_| "AOT weight payload length cannot fit u64. Fix: split the weights artifact before launch.")?;
        if weight_bytes > params_dptr.byte_len() {{
            return Err(format!(
                "AOT weight payload has {{weight_bytes}} byte(s) but the first device buffer allocation has {{}} byte(s). Fix: regenerate the manifest so the parameter/weight buffer covers weights.brotli.",
                params_dptr.byte_len()
            ).into());
        }}
        cuda::cu_memcpy_h_to_d(params_dptr.ptr(), &bundle.weight_bytes)?;
    }}

    {nccl_init}

    let metrics_idx = bundle
        .manifest
        .buffers
        .iter()
        .position(|b| b.name == "metrics");
    let mut kernel_args = cuda::KernelArgs::with_capacity(device_ptrs.len())?;

    {dispatch_block}

    cuda::cu_stream_synchronize()?;

    if let Some(idx) = metrics_idx {{
        if let Some(dptr) = device_ptrs.get(idx) {{
            print_final_metrics(dptr.ptr(), &bundle.manifest)?;
        }}
    }}

    {nccl_drop}

    Ok(())
}}

const DEFAULT_STREAMING_BUFFER_BYTES: u64 = 1 << 24;
const METRIC_RECORD_WORDS: usize = 8;
const CUDA_DEVICE_ORDINAL_ENV: &str = "VYRE_CUDA_DEVICE_ORDINAL";
const TTT_STEPS_ENV: &str = "VYRE_TTT_STEPS";
const TTT_TARGET_LOSS_ENV: &str = "VYRE_TTT_TARGET_LOSS";

fn manifest_buffer_allocation_bytes(
    manifest: &artifact::Manifest,
    index: usize,
) -> Result<u64, Box<dyn std::error::Error>> {{
    let buf = manifest.buffers.get(index).ok_or_else(|| {{
        format!("AOT manifest buffer index {{index}} is out of range. Fix: repair the launcher-generated manifest traversal.")
    }})?;
    let element_count = u64::try_from(buf.element_count)
        .map_err(|_| format!("buffer {{index}} {{:?}} element_count={{}} does not fit u64. Fix: split the AOT manifest buffer or correct the manifest.", buf.name, buf.element_count))?;
    let element_size_bytes = u64::try_from(buf.element_size_bytes)
        .map_err(|_| format!("buffer {{index}} {{:?}} element_size_bytes={{}} does not fit u64. Fix: split the AOT manifest buffer or correct the manifest.", buf.name, buf.element_size_bytes))?;
    let bytes = element_count.checked_mul(element_size_bytes).ok_or_else(|| {{
        format!(
            "buffer {{index}} {{:?}} byte size overflows u64: element_count={{}} element_size_bytes={{}}. Fix: split the AOT manifest buffer or correct the manifest.",
            buf.name,
            buf.element_count,
            buf.element_size_bytes
        )
    }})?;
    Ok(if bytes == 0 {{ DEFAULT_STREAMING_BUFFER_BYTES }} else {{ bytes }})
}}

fn select_cuda_device_ordinal() -> Result<i32, Box<dyn std::error::Error>> {{
    let visible_devices = cuda::cu_device_count()?;
    if visible_devices <= 0 {{
        return Err(format!(
            "CUDA reports {{visible_devices}} visible device(s). Fix: this launcher requires a GPU; repair CUDA_VISIBLE_DEVICES/container GPU passthrough before AOT launch."
        )
        .into());
    }}
    let ordinal = parse_env_i32(CUDA_DEVICE_ORDINAL_ENV, 0)?;
    if ordinal < 0 || ordinal >= visible_devices {{
        return Err(format!(
            "{{CUDA_DEVICE_ORDINAL_ENV}}={{ordinal}} is outside visible CUDA device range 0..{{visible_devices}}. Fix: set {{CUDA_DEVICE_ORDINAL_ENV}} to a visible device ordinal or unset it for ordinal 0."
        )
        .into());
    }}
    Ok(ordinal)
}}

fn validate_manifest_for_launch(
    manifest: &artifact::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {{
    if manifest.entry_point.is_empty() {{
        return Err("AOT manifest entry_point is empty. Fix: regenerate the bundle with a CUDA kernel entry name.".into());
    }}
    if manifest.buffers.is_empty() {{
        return Err("AOT manifest has no buffers; launcher cannot build a CUDA kernel argument table. Fix: regenerate the bundle with at least the parameter/weight buffer.".into());
    }}
    validate_manifest_dispatch_static(manifest)?;
    let mut total_bytes = 0_u64;
    let mut metrics_buffers = 0_usize;
    for (index, buf) in manifest.buffers.iter().enumerate() {{
        let element_size_bytes = u64::try_from(buf.element_size_bytes)
            .map_err(|_| format!("buffer {{index}} {{:?}} element_size_bytes={{}} does not fit u64. Fix: split the AOT manifest buffer or correct the manifest.", buf.name, buf.element_size_bytes))?;
        if buf.name == "metrics" {{
            metrics_buffers += 1;
            if element_size_bytes != 4 {{
                return Err(format!(
                    "metrics buffer at index {{index}} has element_size_bytes={{}} but CUDA AOT metrics are u32 words. Fix: regenerate the manifest with metrics.element_size_bytes=4.",
                    buf.element_size_bytes
                )
                .into());
            }}
            if buf.element_count < METRIC_RECORD_WORDS {{
                return Err(format!(
                    "metrics buffer at index {{index}} has {{}} word(s) but final metrics need at least {{METRIC_RECORD_WORDS}}. Fix: allocate a larger metrics ring in the AOT manifest.",
                    buf.element_count
                )
                .into());
            }}
        }}
        let allocated_bytes = manifest_buffer_allocation_bytes(manifest, index)?;
        total_bytes = total_bytes.checked_add(allocated_bytes).ok_or_else(|| {{
            "AOT manifest aggregate buffer allocation bytes overflow u64. Fix: split the bundle before launch.".to_string()
        }})?;
    }}
    if total_bytes == 0 {{
        return Err("AOT manifest resolved zero aggregate allocation bytes. Fix: regenerate the bundle with real buffers.".into());
    }}
    if metrics_buffers > 1 {{
        return Err(format!(
            "AOT manifest has {{metrics_buffers}} metrics buffers; launcher metrics are ambiguous. Fix: emit exactly one buffer named `metrics`."
        )
        .into());
    }}
    Ok(())
}}

fn validate_manifest_dispatch_static(
    manifest: &artifact::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {{
    for axis in 0..3 {{
        if manifest.dispatch.workgroup_size[axis] == 0 {{
            return Err(format!(
                "AOT manifest workgroup_size axis {{axis}} is zero. Fix: regenerate the bundle with positive CUDA block dimensions."
            )
            .into());
        }}
        if manifest.dispatch.grid_size[axis] == 0 {{
            return Err(format!(
                "AOT manifest grid_size axis {{axis}} is zero, which requires runtime grid derivation not encoded in this launcher. Fix: emit an explicit CUDA grid size or extend the manifest with a concrete runtime-grid source."
            )
            .into());
        }}
    }}
    let threads_per_block = u64::from(manifest.dispatch.workgroup_size[0])
        .checked_mul(u64::from(manifest.dispatch.workgroup_size[1]))
        .and_then(|xy| xy.checked_mul(u64::from(manifest.dispatch.workgroup_size[2])))
        .ok_or_else(|| {{
            format!(
                "AOT manifest workgroup_size {{:?}} overflows u64. Fix: regenerate the bundle with a smaller CUDA block shape.",
                manifest.dispatch.workgroup_size
            )
        }})?;
    if threads_per_block == 0 {{
        return Err("AOT manifest resolved zero threads per block. Fix: regenerate the bundle with a positive CUDA block shape.".into());
    }}
    let grid_blocks = u64::from(manifest.dispatch.grid_size[0])
        .checked_mul(u64::from(manifest.dispatch.grid_size[1]))
        .and_then(|xy| xy.checked_mul(u64::from(manifest.dispatch.grid_size[2])))
        .ok_or_else(|| {{
            format!(
                "AOT manifest grid_size {{:?}} overflows u64. Fix: shard the dispatch or regenerate the bundle with a smaller CUDA grid.",
                manifest.dispatch.grid_size
            )
        }})?;
    if grid_blocks == 0 {{
        return Err("AOT manifest resolved zero CUDA grid blocks. Fix: regenerate the bundle with a positive CUDA grid shape.".into());
    }}
    Ok(())
}}

fn launch_manifest_kernel(
    kernel: cuda::CUfunction,
    bundle: &artifact::LoadedBundle,
    device_ptrs: &[cuda::DeviceAllocation],
    kernel_args: &mut cuda::KernelArgs,
    launch_limits: &cuda::DeviceLaunchLimits,
) -> Result<(), Box<dyn std::error::Error>> {{
    cuda::cu_launch_kernel_prepared(
        kernel,
        bundle.manifest.dispatch.grid_size,
        bundle.manifest.dispatch.workgroup_size,
        bundle.manifest.dispatch.dynamic_shared_bytes,
        device_ptrs,
        kernel_args,
        launch_limits,
    )?;
    Ok(())
}}

fn run_eval_time_training_loop(
    kernel: cuda::CUfunction,
    bundle: &artifact::LoadedBundle,
    device_ptrs: &[cuda::DeviceAllocation],
    metrics_idx: Option<usize>,
    kernel_args: &mut cuda::KernelArgs,
    launch_limits: &cuda::DeviceLaunchLimits,
) -> Result<(), Box<dyn std::error::Error>> {{
    let steps = parse_env_u32(TTT_STEPS_ENV, 1)?;
    if steps == 0 {{
        return Err(format!("{{TTT_STEPS_ENV}}=0 disables the TTT loop. Fix: unset {{TTT_STEPS_ENV}} for one CUDA training step or set it to a positive count.").into());
    }}
    let target_loss = parse_env_optional_f32(TTT_TARGET_LOSS_ENV)?;
    let metrics_dptr = metrics_idx.and_then(|idx| device_ptrs.get(idx).map(|allocation| allocation.ptr()));
    if target_loss.is_some() && metrics_dptr.is_none() {{
        return Err(format!("{{TTT_TARGET_LOSS_ENV}} requires a `metrics` buffer in the AOT manifest. Fix: add a metrics buffer or unset {{TTT_TARGET_LOSS_ENV}}.").into());
    }}
    let sync_for_step_metrics = target_loss.is_some();

    for launch_step in 0..steps {{
        launch_manifest_kernel(kernel, bundle, device_ptrs, kernel_args, launch_limits)?;
        if sync_for_step_metrics {{
            cuda::cu_stream_synchronize()?;
            if let (Some(target), Some(dptr)) = (target_loss, metrics_dptr) {{
                let (metric_step, loss, tokens) = read_final_metric_record(dptr, &bundle.manifest)?;
                if loss.is_finite() && loss <= target {{
                    let completed_step = launch_step + 1;
                    println!("TTT_CONVERGED launch_step={{completed_step}} metric_step={{metric_step}} loss={{loss:.6}} tokens={{tokens}}");
                    return Ok(());
                }}
            }}
        }}
    }}
    Ok(())
}}

fn print_final_metrics(
    metrics_dptr: u64,
    manifest: &artifact::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {{
    let (step, loss, tokens) = read_final_metric_record(metrics_dptr, manifest)?;
    println!("FINAL step={{step}} loss={{loss:.6}} tokens={{tokens}}");
    Ok(())
}}

fn read_final_metric_record(
    metrics_dptr: u64,
    manifest: &artifact::Manifest,
) -> Result<(u32, f32, u32), Box<dyn std::error::Error>> {{
    let metrics_buf = manifest.buffers.iter().find(|b| b.name == "metrics").ok_or(
        "final metrics were requested but the AOT manifest has no `metrics` buffer. Fix: add a metrics buffer or disable metrics readback.",
    )?;
    if metrics_buf.element_size_bytes != 4 {{
        return Err(format!(
            "metrics buffer has element_size_bytes={{}} but final metric records are u32 words. Fix: regenerate the manifest with metrics.element_size_bytes=4.",
            metrics_buf.element_size_bytes
        )
        .into());
    }}
    let ring_size = usize::try_from(metrics_buf.element_count)
        .map_err(|_| format!("metrics buffer element_count={{}} does not fit host usize. Fix: split the metrics ring or regenerate the AOT manifest with a bounded metrics buffer.", metrics_buf.element_count))?;
    if ring_size < METRIC_RECORD_WORDS {{
        return Err(format!(
            "metrics buffer has {{ring_size}} words but final record needs {{METRIC_RECORD_WORDS}}. Fix: allocate a larger metrics ring in the AOT manifest."
        )
        .into());
    }}
    let last_record_offset = (ring_size - METRIC_RECORD_WORDS)
        .checked_mul(4)
        .ok_or("metrics final-record byte offset overflowed. Fix: split the metrics ring or correct the AOT manifest.")?;
    let last_record_offset = u64::try_from(last_record_offset)
        .map_err(|_| "metrics final-record byte offset exceeds u64. Fix: split the metrics ring or correct the AOT manifest.")?;

    let mut record = [0u32; METRIC_RECORD_WORDS];
    cuda::cu_memcpy_d_to_h_offset(&mut record, metrics_dptr, last_record_offset)?;

    let step = record[0];
    let loss = f32::from_bits(record[1]);
    let tokens = record[2];
    Ok((step, loss, tokens))
}}
"##,
        nccl_use = nccl_use,
        nccl_init = nccl_init,
        nccl_drop = nccl_drop,
        dispatch_block = dispatch_block,
    )
}

#[cfg(test)]

mod tests {
    #[test]
    fn ttt_loop_does_not_sync_every_step_without_metric_readback() {
        let source = include_str!("aot_launcher.rs");
        assert!(
            source.contains("let sync_for_step_metrics = target_loss.is_some();"),
            "Fix: generated CUDA AOT TTT loops must distinguish metric-readback launches from firehose launches."
        );
        assert!(
            source.contains("if sync_for_step_metrics {{\n            cuda::cu_stream_synchronize()?;"),
            "Fix: generated CUDA AOT TTT loops must only fence per step when target-loss readback needs metrics."
        );
        assert!(
            !source.contains(concat!(
                "launch_manifest_kernel(kernel, bundle, device_ptrs, kernel_args, launch_limits)?;\n",
                "        cuda::cu_stream_synchronize()?;\n",
                "        if let (Some(target), Some(dptr))"
            )),
            "Fix: generated CUDA AOT TTT loops must not synchronize after every launch when no metric target is configured."
        );
    }

    use super::*;
    use vyre_driver::aot::AotLauncherRequest;

    fn request(include_ttt_loop: bool) -> AotLauncherRequest<'static> {
        AotLauncherRequest {
            target: "secondary_text",
            crate_name: "vyre_cuda_launcher_test",
            include_collectives: false,
            include_ttt_loop,
        }
    }

    #[test]
    fn emitted_launcher_preflights_manifest_and_device_limits_before_launch() {
        let main = emit_main(&request(false));

        assert!(
            main.contains("validate_manifest_for_launch(&bundle.manifest)?;"),
            "Fix: generated CUDA AOT launchers must validate manifest buffer/entry contracts before allocating or launching."
        );
        assert!(
            main.contains("let launch_limits = cuda::cu_device_launch_limits(device)?;"),
            "Fix: generated CUDA AOT launchers must query live device launch limits before context launch."
        );
        assert!(
            main.contains("let device_ordinal = select_cuda_device_ordinal()?;")
                && main.contains("let device = cuda::cu_device_get(device_ordinal)?;"),
            "Fix: generated CUDA AOT launchers must select a validated device ordinal instead of hard-coding ordinal 0."
        );
        assert!(
            main.contains("launch_manifest_kernel(kernel, &bundle, &device_ptrs, &mut kernel_args, &launch_limits)?;"),
            "Fix: generated CUDA AOT launchers must pass probed launch limits into every manifest launch."
        );
    }

    #[test]
    fn emitted_launcher_bounds_weight_upload_by_first_allocation() {
        let main = emit_main(&request(false));

        assert!(
            main.contains("let weight_bytes = u64::try_from(bundle.weight_bytes.len())"),
            "Fix: generated CUDA AOT launchers must convert weight payload length before upload accounting."
        );
        assert!(
            main.contains("if weight_bytes > params_dptr.byte_len()"),
            "Fix: generated CUDA AOT launchers must reject weight payloads larger than the parameter allocation."
        );
        assert!(
            main.contains("parameter/weight buffer covers weights.brotli"),
            "Fix: generated CUDA AOT launchers must produce an actionable manifest fix for oversized weights."
        );
    }

    #[test]
    fn emitted_launcher_validates_visible_cuda_device_ordinal() {
        let main = emit_main(&request(false));

        assert!(
            main.contains("const CUDA_DEVICE_ORDINAL_ENV: &str = \"VYRE_CUDA_DEVICE_ORDINAL\";"),
            "Fix: generated CUDA AOT launchers must expose a stable device-ordinal environment override."
        );
        assert!(
            main.contains("let visible_devices = cuda::cu_device_count()?;")
                && main.contains("if visible_devices <= 0"),
            "Fix: generated CUDA AOT launchers must fail loudly when CUDA reports no visible GPU."
        );
        assert!(
            main.contains("ordinal < 0 || ordinal >= visible_devices"),
            "Fix: generated CUDA AOT launchers must reject out-of-range device ordinals before cuDeviceGet."
        );
    }

    #[test]
    fn emitted_launcher_validates_metrics_buffer_abi_before_cuda_launch() {
        let main = emit_main(&request(true));

        assert!(
            main.contains("let mut metrics_buffers = 0_usize;"),
            "Fix: generated CUDA AOT launchers must count metrics buffers during manifest validation."
        );
        assert!(
            main.contains("metrics.element_size_bytes=4"),
            "Fix: generated CUDA AOT launchers must reject metrics buffers that are not u32-word ABI buffers."
        );
        assert!(
            main.contains("buf.element_count < METRIC_RECORD_WORDS"),
            "Fix: generated CUDA AOT launchers must reject undersized metrics rings before kernel execution."
        );
        assert!(
            main.contains("metrics buffer element_count={}") && main.contains("does not fit host usize"),
            "Fix: generated CUDA AOT metrics readback must report an actionable manifest error when element_count cannot fit host indexing."
        );
        assert!(
            main.contains("metrics_buffers > 1"),
            "Fix: generated CUDA AOT launchers must reject ambiguous duplicate metrics buffers."
        );
        assert!(
            !main.contains(".unwrap_or(4096)"),
            "Fix: final metrics readback must not invent a default ring size when the manifest lacks metrics."
        );
    }

    #[test]
    fn emitted_launcher_centralizes_manifest_allocation_byte_math() {
        let main = emit_main(&request(false));

        assert!(
            main.contains("fn manifest_buffer_allocation_bytes("),
            "Fix: generated CUDA AOT launchers must centralize manifest byte-size and zero-buffer allocation policy."
        );
        assert!(
            main.contains("manifest_buffer_allocation_bytes(&bundle.manifest, index)?;"),
            "Fix: generated CUDA AOT allocation must use the same byte calculator as manifest validation."
        );
        assert!(
            main.contains(
                "let allocated_bytes = manifest_buffer_allocation_bytes(manifest, index)?;"
            ),
            "Fix: generated CUDA AOT validation must use the same byte calculator as allocation."
        );
    }

    #[test]
    fn emitted_launcher_rejects_runtime_grid_stub_and_zero_block_shapes() {
        let main = emit_main(&request(false));

        assert!(
            main.contains("validate_manifest_dispatch_static(manifest)?;"),
            "Fix: generated CUDA AOT launchers must statically preflight dispatch geometry during manifest validation."
        );
        assert!(
            main.contains("manifest.dispatch.workgroup_size[axis] == 0"),
            "Fix: generated CUDA AOT launchers must reject zero CUDA block axes before device launch."
        );
        assert!(
            main.contains("runtime grid derivation not encoded in this launcher"),
            "Fix: generated CUDA AOT launchers must fail loudly for grid_size=0 instead of pretending one block covers runtime-sized work."
        );
        assert!(
            CUDA_FFI.contains("if grid[axis] == 0 || grid[axis] > limits.max_grid_dim[axis]"),
            "Fix: generated CUDA FFI must not silently rewrite zero grid axes to one."
        );
        assert!(
            !CUDA_FFI.contains("if grid[0] == 0 { 1 } else { grid[0] }"),
            "Fix: generated CUDA FFI must remove the zero-grid-to-one launch stub."
        );
    }

    #[test]
    fn emitted_ttt_loop_reuses_launch_preflight_for_every_step() {
        let main = emit_main(&request(true));

        assert!(
            main.contains("run_eval_time_training_loop(kernel, &bundle, &device_ptrs, metrics_idx, &mut kernel_args, &launch_limits)?;"),
            "Fix: generated CUDA TTT launchers must pass launch limits into the repeated training loop."
        );
        assert!(
            main.contains("launch_manifest_kernel(kernel, bundle, device_ptrs, kernel_args, launch_limits)?;"),
            "Fix: every CUDA TTT loop iteration must reuse the same device-limit preflight instead of calling cuLaunchKernel directly."
        );
    }

    #[test]
    fn cuda_ffi_template_rejects_null_pointers_and_checked_allocations() {
        assert!(
            CUDA_FFI.contains("usize::try_from(requested_bytes)"),
            "Fix: generated CUDA FFI must not cast u64 allocation sizes into usize."
        );
        assert!(
            CUDA_FFI.contains("if dptr == 0"),
            "Fix: generated CUDA FFI must reject null device pointers returned after allocation success."
        );
        assert!(
            CUDA_FFI.contains("src.checked_add(offset_bytes)"),
            "Fix: generated CUDA FFI must check device-pointer offset arithmetic before readback."
        );
        assert!(
            CUDA_FFI.contains("if src.is_empty() {\n        return Ok(());\n    }")
                && CUDA_FFI.contains("if dst.is_empty() {\n        return Ok(());\n    }"),
            "Fix: generated CUDA FFI copy wrappers must preserve runtime zero-byte no-op behavior."
        );
        assert!(
            CUDA_FFI.contains("let bytes_u64 = u64::try_from(bytes)")
                && CUDA_FFI.contains("src.checked_add(bytes_u64)"),
            "Fix: generated CUDA FFI offset readback must validate the full start..start+byte_len device range."
        );
        assert!(
            CUDA_FFI.contains("AOT CUDA kernel argument {index} is a null device pointer"),
            "Fix: generated CUDA FFI must reject null kernel arguments before cuLaunchKernel."
        );
    }

    #[test]
    fn cuda_ffi_template_uses_raii_for_modules_and_device_allocations() {
        assert!(
            CUDA_FFI.contains("pub struct ModuleGuard"),
            "Fix: generated CUDA FFI must own loaded modules and unload them on drop."
        );
        assert!(
            CUDA_FFI.contains("cuModuleUnload"),
            "Fix: generated CUDA FFI ModuleGuard must call cuModuleUnload."
        );
        assert!(
            CUDA_FFI.contains("pub struct DeviceAllocation"),
            "Fix: generated CUDA FFI must represent device allocations as owned resources."
        );
        assert!(
            CUDA_FFI.contains("cuMemFree_v2"),
            "Fix: generated CUDA FFI DeviceAllocation must call cuMemFree_v2 on drop."
        );
        let main = emit_main(&request(false));
        assert!(
            main.contains("Vec<cuda::DeviceAllocation>"),
            "Fix: generated launcher main must store owned CUDA allocations, not raw u64 pointers."
        );
        assert!(
            main.contains("cuda::cu_module_get_function(&module"),
            "Fix: generated launcher main must keep ModuleGuard alive while resolving the kernel function."
        );
    }

    #[test]
    fn cuda_ffi_template_rejects_null_context_success_and_drops_safely() {
        let context_owner = CUDA_FFI
            .split("pub struct CtxGuard")
            .nth(1)
            .and_then(|tail| tail.split("pub struct ModuleGuard").next())
            .expect("Fix: generated CUDA FFI must keep context ownership before module ownership.");

        assert!(
            context_owner.contains("if self.raw.is_null() {\n            return;\n        }"),
            "Fix: generated CUDA context guard Drop must not call cuCtxDestroy_v2 on a null context."
        );
        assert!(
            context_owner.contains("if ctx.is_null()")
                && context_owner.contains("cuCtxCreate_v2 returned a null context after success"),
            "Fix: generated CUDA context creation must reject null-success handles before module load."
        );
    }

    #[test]
    fn cuda_ffi_template_has_no_release_path_unwrap_or_panic_stubs() {
        for forbidden in [
            concat!(".", "unwrap()"),
            "Vec::with_capacity",
            "bytes.to_vec()",
            concat!("panic", "!("),
            concat!("todo", "!("),
            concat!("unimplemented", "!("),
        ] {
            assert!(
                !CUDA_FFI.contains(forbidden),
                "Fix: generated CUDA FFI is release-path code and must return actionable errors instead of using {forbidden}."
            );
        }
        assert!(
            CUDA_FFI.contains("c\"libcuda.so.1\"")
                && CUDA_FFI.contains("CString::new($name).map_err"),
            "Fix: generated CUDA FFI must construct driver library and symbol names without unwrap()."
        );
        assert!(
            CUDA_FFI.contains("if bytes.ends_with(&[0])")
                && CUDA_FFI.contains("reserve_vec_to_capacity(")
                && CUDA_FFI.contains("module_image_ptr"),
            "Fix: generated CUDA FFI must borrow already-NUL-terminated PTX and fallibly stage only when a terminator is missing."
        );
        let main = emit_main(&request(false));
        assert!(
            main.contains("fn reserve_vec_to_capacity<T>")
                && main.contains("AOT device allocation table"),
            "Fix: generated CUDA AOT launchers must fallibly reserve the device-allocation table before CUDA allocation."
        );
        assert!(
            main.contains("cuda::KernelArgs::with_capacity(device_ptrs.len())?"),
            "Fix: generated CUDA AOT launchers must propagate fallible kernel-argument table reservation."
        );
    }

    #[test]
    fn emitted_launcher_uses_driver_file_container_constructor() {
        let source = include_str!("aot_launcher.rs");
        assert!(
            !source.contains(concat!("BTree", "Map")),
            "Fix: CUDA launcher emission must not open-code ordered map assembly; centralize the public file container in vyre-driver."
        );
        assert!(
            source.contains("reserve_vec(&mut entries, file_count"),
            "Fix: CUDA launcher emission must fallibly reserve its fixed file-entry list before staging generated source."
        );
        assert!(
            source.contains("AotLauncherFiles::from_entries"),
            "Fix: CUDA launcher emission must use the backend-neutral constructor for launcher file containers."
        );
    }
}
