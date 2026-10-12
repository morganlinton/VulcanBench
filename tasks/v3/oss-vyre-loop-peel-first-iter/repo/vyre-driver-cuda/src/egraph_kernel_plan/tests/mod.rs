mod args_contracts;
mod planner_contracts;
mod ptx_contracts;
mod signature_bucket_contracts;
mod source_shape_contracts;
mod structural_equivalence_contracts;

use super::*;
use crate::egraph_kernel_plan::args::{
    EGraphCanonicalRewriteKernelArgs, EGraphStructuralKernelArgs,
};
use crate::plan_cuda_egraph_device_upload;
use crate::CudaEGraphDeviceKernelView;
use vyre_foundation::optimizer::eqsat_gpu::GpuEGraphSnapshot;
use vyre_foundation::optimizer::eqsat_gpu::{Equivalence, GpuEGraphDeviceImage};

/// Production source of the e-graph kernel planner, concatenated across the
/// aggregator and every submodule (the `fast-path module splits` refactor
/// moved helpers out of `egraph_kernel_plan.rs` into submodules). Each file's
/// `#[cfg(test)]` tail is stripped, so source-scan contracts see only release
/// code and counts are preserved regardless of which submodule owns a helper.
fn planner_production_source() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut sources =
        vec![
            std::fs::read_to_string(format!("{manifest}/src/egraph_kernel_plan.rs"))
                .unwrap_or_default(),
        ];
    if let Ok(entries) = std::fs::read_dir(format!("{manifest}/src/egraph_kernel_plan")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path.file_name().and_then(|n| n.to_str()) != Some("tests.rs")
            {
                sources.push(std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    sources
        .iter()
        .map(|s| s.split("#[cfg(test)]").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn synthetic_view(rows: usize, children: usize, groups: usize) -> CudaEGraphDeviceKernelView {
    assert!(groups <= rows);
    assert!(children <= rows.saturating_mul(2));
    let mut child_storage = Vec::new();
    let mut row_specs = Vec::with_capacity(rows);
    for row in 0..rows {
        let start = child_storage.len();
        if child_storage.len() < children && row > 0 {
            child_storage.push((row - 1) as u32);
        }
        if child_storage.len() < children && row > 1 {
            child_storage.push((row / 2) as u32);
        }
        let eclass = if groups == 0 { row } else { row % groups };
        row_specs.push((
            eclass as u32,
            if row & 1 == 0 { "lit" } else { "add" },
            start,
            child_storage.len() - start,
        ));
    }
    while child_storage.len() < children {
        child_storage.push(0);
        let last = row_specs
            .last_mut()
            .expect("Fix: synthetic child-only view requires at least one row");
        last.3 += 1;
    }
    let build_rows = row_specs
        .iter()
        .map(|&(class, op, start, len)| (class, op, &child_storage[start..start + len]))
        .collect::<Vec<_>>();
    let snapshot = GpuEGraphSnapshot::build(build_rows);
    let plan = plan_cuda_egraph_device_upload(&snapshot).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - synthetic plan must pack");
    CudaEGraphDeviceKernelView::from_checked_parts(0x1000, plan.byte_len(), plan.byte_layout())
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - synthetic view must be valid")
}

fn view_for_image(image: &GpuEGraphDeviceImage) -> CudaEGraphDeviceKernelView {
    let plan = crate::plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - packed egraph image must have a CUDA upload plan");
    CudaEGraphDeviceKernelView::from_checked_parts(0x4000, plan.byte_len(), plan.byte_layout())
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - upload plan must resolve to a checked kernel view")
}

