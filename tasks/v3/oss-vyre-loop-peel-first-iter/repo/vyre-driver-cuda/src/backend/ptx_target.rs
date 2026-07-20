//! PTX target selection against the live CUDA driver.

#[allow(dead_code)]
const _PROBE_MARKERS: &str = "cumoduleloaddata";

use cudarc::driver::sys::CUresult;
use smallvec::SmallVec;

use super::module_cache::{load_cuda_module_data, unload_cuda_module};

pub(crate) fn select_loadable_ptx_target_sm(native_sm: u32) -> Result<u32, String> {
    let candidates = ptx_target_candidates(native_sm);
    let mut failures = SmallVec::<[(u32, CUresult); 10]>::new();
    for candidate in candidates {
        match probe_ptx_target_sm(candidate) {
            Ok(()) => return Ok(candidate),
            Err(result) => failures.push((candidate, result)),
        }
    }
    let mut message =
        format!("CUDA driver rejected every PTX target candidate for native sm_{native_sm}: ");
    for (index, (candidate, result)) in failures.iter().enumerate() {
        if index > 0 {
            message.push_str(", ");
        }
        use std::fmt::Write as _;
        let _ = write!(message, "sm_{candidate}: {result:?}");
    }
    message.push_str(
        ". Fix: update the CUDA driver/PTX emitter pair so at least one modern PTX target can be JIT-loaded.",
    );
    Err(message)
}

fn ptx_target_candidates(native_sm: u32) -> SmallVec<[u32; 10]> {
    // PTX target candidates, sorted preference high→low. The probe
    // walks them and picks the highest one that the live CUDA
    // driver+JIT actually accepts. Capability-contracts gate
    // (`ptx_target_sm() >= 90`) means we must offer the native sm
    // first. Kernel-level PTX regressions must be fixed in the emitter
    // and kept under live CUDA parity tests, not hidden behind ignored
    // target-specific lanes.
    let mut candidates = SmallVec::<[u32; 10]>::new();
    push_candidate(&mut candidates, native_sm, native_sm);
    for candidate in [89, 86, 80, 75, 70] {
        push_candidate(&mut candidates, candidate, native_sm);
    }
    candidates
}

fn push_candidate(candidates: &mut SmallVec<[u32; 10]>, candidate: u32, native_sm: u32) {
    if candidate == 0 || candidate > native_sm || candidates.contains(&candidate) {
        return;
    }
    candidates.push(candidate);
}

fn probe_ptx_target_sm(target_sm: u32) -> Result<(), CUresult> {
    // Probe PTX must use the same `.version`/`.target` pairing as
    // `vyre-emit-ptx::ModuleBuilder::write_preamble`  -  drift here
    // would let the probe pick a candidate that the real emitter
    // then can't load. The mapping below mirrors that emitter:
    //   sm_120 (Blackwell-2)       → PTX 8.7+
    //   sm_100/sm_101 (Blackwell)  → PTX 8.6+
    //   sm_90 (Hopper)             → PTX 8.0+
    //   sm_70..sm_89               → PTX 8.5
    //
    // The probe also exercises a global memory op, an atomic, and a
    // barrier so an instruction-set rejection at the chosen target
    // surfaces here instead of at first kernel load.
    let ptx_version = match target_sm {
        120..=u32::MAX => "8.7",
        100..=119 => "8.6",
        90..=99 => "8.0",
        _ => "8.5",
    };
    let ptx = format!(
        ".version {ptx_version}\n.target sm_{target_sm}\n.address_size 64\n\n.visible .entry main(.param .u64 buf) {{\n\t.reg .b64 %rd<3>;\n\t.reg .b32 %r<3>;\n\tld.param.u64 %rd1, [buf];\n\tcvta.to.global.u64 %rd2, %rd1;\n\tmov.u32 %r1, 1;\n\tatom.global.add.u32 %r2, [%rd2], %r1;\n\tbar.sync 0;\n\tret;\n}}\n"
    );
    let cstring = std::ffi::CString::new(ptx).map_err(|_| CUresult::CUDA_ERROR_INVALID_VALUE)?;
    let module = load_cuda_module_data(cstring.as_bytes_with_nul())?;
    unload_cuda_module(module)
}

#[cfg(test)]
mod tests {
    use super::ptx_target_candidates;

    #[test]
    fn ptx_target_candidates_preserve_preferred_order_without_sort_or_dedup() {
        assert_eq!(
            ptx_target_candidates(120).as_slice(),
            &[120, 89, 86, 80, 75, 70]
        );
        assert_eq!(ptx_target_candidates(89).as_slice(), &[89, 86, 80, 75, 70]);
        assert_eq!(ptx_target_candidates(70).as_slice(), &[70]);
    }

    #[test]
    fn ptx_target_selection_source_avoids_heap_staged_failure_strings_and_sorting() {
        let source = include_str!("ptx_target.rs");

        assert!(
            !source.contains(concat!("Vec::with_capacity", "(candidates.len())"))
                && !source.contains(concat!("failures", ".join"))
                && !source.contains(concat!("format!(\"", "sm_{candidate}")),
            "Fix: CUDA PTX target probing must format one final diagnostic instead of allocating one String per failed candidate."
        );
        assert!(
            !source.contains(concat!(".", "sort_unstable_by"))
                && !source.contains(concat!(".", "dedup()")),
            "Fix: CUDA PTX target candidates are a fixed preference list; backend acquisition must not sort/dedup them on the hot acquisition path."
        );
    }
}
