//! CUDA driver-backed execution tests for the NVRTC compile gate.

//! Real CUDA driver PTX module-load gate.
//!
//! Enabled only when the `nvrtc` feature is active because it
//! requires a CUDA driver and toolkit at test time. The feature is
//! off by default so CI and environments without CUDA can still run
//! the mock gate tests above.

use std::ffi::CString;

use super::fixtures::{
    ptx_for_dynamic_vector_load_fusion, ptx_for_dynamic_vector_store_fusion, ptx_for_op,
    ptx_for_vector_load_fusion, ptx_for_vector_store_fusion,
};
use cudarc::driver::{sys::CUresult, CudaContext, LaunchConfig, PushKernelArg};
use cudarc::nvrtc::Ptx;
use vyre_foundation::ir::BinOp;
use vyre_lower::KernelOpKind;

fn driver_loads_ptx(ptx: &str) -> Result<(), String> {
    let ctx =
        CudaContext::new(0).map_err(|error| format!("CUDA context creation failed: {error}"))?;
    ctx.bind_to_thread()
        .map_err(|error| format!("CUDA context bind failed: {error}"))?;
    let ptx = CString::new(ptx)
        .map_err(|error| format!("emitted PTX contained interior NUL: {error}"))?;
    let mut module = std::ptr::null_mut();
    let load = unsafe { cudarc::driver::sys::cuModuleLoadData(&mut module, ptx.as_ptr().cast()) };
    if load != CUresult::CUDA_SUCCESS {
        return Err(format!("cuModuleLoadData failed with {load:?}"));
    }
    if !module.is_null() {
        let unload = unsafe { cudarc::driver::sys::cuModuleUnload(module) };
        if unload != CUresult::CUDA_SUCCESS {
            return Err(format!("cuModuleUnload failed with {unload:?}"));
        }
    }
    Ok(())
}

fn launch_vector_load_fusion_ptx(ptx: &str) -> Result<Vec<u32>, String> {
    launch_vector_load_fusion_matrix(ptx, &[[7_u32, 11, 13, 17]])
}

fn launch_vector_load_fusion_matrix(ptx: &str, cases: &[[u32; 4]]) -> Result<Vec<u32>, String> {
    let ctx =
        CudaContext::new(0).map_err(|error| format!("CUDA context creation failed: {error}"))?;
    let stream = ctx.default_stream();
    let module = ctx
        .load_module(Ptx::from_src(ptx))
        .map_err(|error| format!("CUDA module load failed: {error}"))?;
    let kernel = module
        .load_function("main")
        .map_err(|error| format!("CUDA function lookup failed: {error}"))?;
    let params = stream
        .clone_htod(&[1_u32, 4, 1])
        .map_err(|error| format!("params HtoD copy failed: {error}"))?;

    let mut observed = Vec::with_capacity(cases.len());
    for input_host in cases {
        let input = stream
            .clone_htod(input_host)
            .map_err(|error| format!("input HtoD copy failed: {error}"))?;
        let mut output = stream
            .alloc_zeros::<u32>(1)
            .map_err(|error| format!("output allocation failed: {error}"))?;

        unsafe {
            stream
                .launch_builder(&kernel)
                .arg(&input)
                .arg(&mut output)
                .arg(&params)
                .launch(LaunchConfig::for_num_elems(1))
        }
        .map_err(|error| format!("vector load kernel launch failed: {error}"))?;

        let output_host = stream
            .clone_dtoh(&output)
            .map_err(|error| format!("output DtoH copy failed: {error}"))?;
        observed.push(output_host[0]);
    }
    Ok(observed)
}

fn launch_dynamic_vector_load_fusion_ptx(
    ptx: &str,
    input_host: &[u32],
    thread_count: usize,
) -> Result<Vec<u32>, String> {
    if input_host.len() != thread_count * 4 {
        return Err(format!(
            "input length {} must equal thread_count * 4 ({})",
            input_host.len(),
            thread_count * 4
        ));
    }
    let ctx =
        CudaContext::new(0).map_err(|error| format!("CUDA context creation failed: {error}"))?;
    let stream = ctx.default_stream();
    let module = ctx
        .load_module(Ptx::from_src(ptx))
        .map_err(|error| format!("CUDA module load failed: {error}"))?;
    let kernel = module
        .load_function("main")
        .map_err(|error| format!("CUDA function lookup failed: {error}"))?;

    let input = stream
        .clone_htod(input_host)
        .map_err(|error| format!("input HtoD copy failed: {error}"))?;
    let mut output = stream
        .alloc_zeros::<u32>(thread_count)
        .map_err(|error| format!("output allocation failed: {error}"))?;
    let params = stream
        .clone_htod(&[
            thread_count as u32,
            input_host.len() as u32,
            thread_count as u32,
        ])
        .map_err(|error| format!("params HtoD copy failed: {error}"))?;

    unsafe {
        stream
            .launch_builder(&kernel)
            .arg(&input)
            .arg(&mut output)
            .arg(&params)
            .launch(LaunchConfig::for_num_elems(thread_count as u32))
    }
    .map_err(|error| format!("dynamic vector load kernel launch failed: {error}"))?;

    stream
        .clone_dtoh(&output)
        .map_err(|error| format!("output DtoH copy failed: {error}"))
}

fn launch_vector_store_fusion_ptx(ptx: &str) -> Result<Vec<u32>, String> {
    let outputs = launch_vector_store_fusion_matrix(ptx, &[[0_u32, 0, 0, 0]])?;
    Ok(outputs[0].to_vec())
}

fn launch_vector_store_fusion_matrix(
    ptx: &str,
    initial_outputs: &[[u32; 4]],
) -> Result<Vec<[u32; 4]>, String> {
    let ctx =
        CudaContext::new(0).map_err(|error| format!("CUDA context creation failed: {error}"))?;
    let stream = ctx.default_stream();
    let module = ctx
        .load_module(Ptx::from_src(ptx))
        .map_err(|error| format!("CUDA module load failed: {error}"))?;
    let kernel = module
        .load_function("main")
        .map_err(|error| format!("CUDA function lookup failed: {error}"))?;
    let params = stream
        .clone_htod(&[1_u32, 4])
        .map_err(|error| format!("params HtoD copy failed: {error}"))?;

    let mut observed = Vec::with_capacity(initial_outputs.len());
    for initial_output in initial_outputs {
        let mut output = stream
            .clone_htod(initial_output)
            .map_err(|error| format!("output HtoD copy failed: {error}"))?;

        unsafe {
            stream
                .launch_builder(&kernel)
                .arg(&mut output)
                .arg(&params)
                .launch(LaunchConfig::for_num_elems(1))
        }
        .map_err(|error| format!("vector store kernel launch failed: {error}"))?;

        let output_host = stream
            .clone_dtoh(&output)
            .map_err(|error| format!("output DtoH copy failed: {error}"))?;
        observed.push(
            output_host
                .try_into()
                .map_err(|output: Vec<u32>| format!("expected 4 lanes, got {}", output.len()))?,
        );
    }
    Ok(observed)
}

fn launch_dynamic_vector_store_fusion_ptx(
    ptx: &str,
    thread_count: usize,
) -> Result<Vec<u32>, String> {
    let ctx =
        CudaContext::new(0).map_err(|error| format!("CUDA context creation failed: {error}"))?;
    let stream = ctx.default_stream();
    let module = ctx
        .load_module(Ptx::from_src(ptx))
        .map_err(|error| format!("CUDA module load failed: {error}"))?;
    let kernel = module
        .load_function("main")
        .map_err(|error| format!("CUDA function lookup failed: {error}"))?;
    let mut output = stream
        .alloc_zeros::<u32>(thread_count * 4)
        .map_err(|error| format!("output allocation failed: {error}"))?;
    let params = stream
        .clone_htod(&[thread_count as u32, (thread_count * 4) as u32])
        .map_err(|error| format!("params HtoD copy failed: {error}"))?;

    unsafe {
        stream
            .launch_builder(&kernel)
            .arg(&mut output)
            .arg(&params)
            .launch(LaunchConfig::for_num_elems(thread_count as u32))
    }
    .map_err(|error| format!("dynamic vector store kernel launch failed: {error}"))?;

    stream
        .clone_dtoh(&output)
        .map_err(|error| format!("output DtoH copy failed: {error}"))
}

fn vector_load_runtime_cases() -> Vec<[u32; 4]> {
    (0_u32..64)
        .map(|seed| {
            let mixed = seed.wrapping_mul(0x9e37_79b1).rotate_left(seed % 31);
            [
                mixed,
                mixed.wrapping_add(seed.wrapping_mul(3).wrapping_add(1)),
                mixed ^ 0xa5a5_5a5a,
                mixed.wrapping_mul(7).wrapping_add(0x1234_5678),
            ]
        })
        .collect()
}

fn vector_store_initial_outputs() -> Vec<[u32; 4]> {
    (0_u32..64)
        .map(|seed| {
            let sentinel = 0xfeed_0000_u32.wrapping_add(seed);
            [
                sentinel,
                sentinel.rotate_left(5),
                sentinel ^ 0xffff_0000,
                sentinel.wrapping_mul(17),
            ]
        })
        .collect()
}

fn dynamic_vector_load_input(thread_count: usize) -> Vec<u32> {
    (0..thread_count * 4)
        .map(|idx| {
            let value = idx as u32;
            value
                .wrapping_mul(0x045d_9f3b)
                .rotate_left((value % 17) + 1)
                ^ 0x9e37_79b9
        })
        .collect()
}

fn dynamic_vector_store_expected(thread_count: usize) -> Vec<u32> {
    (0..thread_count)
        .flat_map(|thread| {
            let thread = thread as u32;
            [
                thread.wrapping_add(1000),
                thread.wrapping_add(1001),
                thread.wrapping_add(1002),
                thread.wrapping_add(1003),
            ]
        })
        .collect()
}

#[test]
fn nvrtc_compiles_add_ptx() {
    let ptx = ptx_for_op(KernelOpKind::BinOpKind(BinOp::Add));
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("add"), "missing add instruction");
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load add PTX"
    );
}

#[test]
fn nvrtc_compiles_mul_ptx() {
    let ptx = ptx_for_op(KernelOpKind::BinOpKind(BinOp::Mul));
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("mul.lo"), "missing mul.lo instruction");
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load mul PTX"
    );
}

#[test]
fn nvrtc_compiles_fma_ptx() {
    let ptx = ptx_for_op(KernelOpKind::Fma);
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("fma.rn"), "missing fma.rn instruction");
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load fma PTX"
    );
}

#[test]
fn nvrtc_compiles_vector_load_fusion_ptx() {
    let ptx = ptx_for_vector_load_fusion();
    assert!(
        ptx.contains("ld.global.nc.v4.u32") || ptx.contains("ld.global.v4.u32"),
        "missing fused vector load"
    );
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load vector-load fusion PTX"
    );
}

#[test]
fn nvrtc_compiles_dynamic_vector_load_fusion_ptx() {
    let ptx = ptx_for_dynamic_vector_load_fusion();
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load dynamic vector-load fusion PTX"
    );
}

#[test]
fn nvrtc_compiles_vector_store_fusion_ptx() {
    let ptx = ptx_for_vector_store_fusion();
    assert!(
        ptx.contains("st.global.v4.u32") || ptx.contains("st.global.nc.v4.u32"),
        "missing fused vector store"
    );
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load vector-store fusion PTX"
    );
}

#[test]
fn nvrtc_compiles_dynamic_vector_store_fusion_ptx() {
    let ptx = ptx_for_dynamic_vector_store_fusion();
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert_eq!(
        driver_loads_ptx(&ptx),
        Ok(()),
        "CUDA driver must load dynamic vector-store fusion PTX"
    );
}

#[test]
fn nvrtc_executes_vector_load_fusion_ptx() {
    let ptx = ptx_for_vector_load_fusion();
    let output = launch_vector_load_fusion_ptx(&ptx);
    assert_eq!(
        output,
        Ok(vec![48]),
        "CUDA execution must match CPU sum of fused vector-loaded values"
    );
}

#[test]
fn nvrtc_executes_vector_store_fusion_ptx() {
    let ptx = ptx_for_vector_store_fusion();
    let output = launch_vector_store_fusion_ptx(&ptx);
    assert_eq!(
        output,
        Ok(vec![10, 11, 12, 13]),
        "CUDA execution must materialize all lanes from fused vector store"
    );
}

#[test]
fn nvrtc_executes_vector_load_fusion_runtime_matrix() {
    let ptx = ptx_for_vector_load_fusion();
    let cases = vector_load_runtime_cases();
    let expected = cases
        .iter()
        .map(|case| {
            case.iter()
                .copied()
                .fold(0_u32, |acc, value| acc.wrapping_add(value))
        })
        .collect::<Vec<_>>();
    let output = launch_vector_load_fusion_matrix(&ptx, &cases);
    assert_eq!(
        output,
        Ok(expected),
        "CUDA execution must match wrapping CPU sums across the vector-load matrix"
    );
}

#[test]
fn nvrtc_executes_dynamic_vector_load_fusion_multithread_matrix() {
    let ptx = ptx_for_dynamic_vector_load_fusion();
    let thread_count = 64;
    let input = dynamic_vector_load_input(thread_count);
    let expected = input
        .chunks_exact(4)
        .map(|chunk| {
            chunk
                .iter()
                .copied()
                .fold(0_u32, |acc, value| acc.wrapping_add(value))
        })
        .collect::<Vec<_>>();
    let output = launch_dynamic_vector_load_fusion_ptx(&ptx, &input, thread_count);
    assert_eq!(
        output,
        Ok(expected),
        "CUDA execution must match CPU chunk sums for dynamic-base fused vector loads"
    );
}

#[test]
fn nvrtc_executes_vector_store_fusion_overwrite_matrix() {
    let ptx = ptx_for_vector_store_fusion();
    let initial_outputs = vector_store_initial_outputs();
    let expected = vec![[10_u32, 11, 12, 13]; initial_outputs.len()];
    let output = launch_vector_store_fusion_matrix(&ptx, &initial_outputs);
    assert_eq!(
        output,
        Ok(expected),
        "CUDA execution must overwrite every sentinel lane through the fused vector store"
    );
}

#[test]
fn nvrtc_executes_dynamic_vector_store_fusion_multithread_matrix() {
    let ptx = ptx_for_dynamic_vector_store_fusion();
    let thread_count = 64;
    let expected = dynamic_vector_store_expected(thread_count);
    let output = launch_dynamic_vector_store_fusion_ptx(&ptx, thread_count);
    assert_eq!(
        output,
        Ok(expected),
        "CUDA execution must materialize every dynamic-base fused vector-store lane"
    );
}
