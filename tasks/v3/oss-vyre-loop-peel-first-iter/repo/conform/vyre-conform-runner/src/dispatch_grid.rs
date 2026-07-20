//! Dispatch-grid selection shared by conform certificate paths.

use vyre::ir::{BufferAccess, Program};
use vyre::DispatchConfig;
use vyre_test_harness::fp_parity::f32_ulp_tolerance;

/// Build the dispatch config required by a program's workgroup shape.
///
/// Concrete dispatch backends derive a 1D grid from writable output length only when a
/// program has a 1D workgroup. Non-1D fixtures need an explicit logical grid;
/// the current conform fixtures are single-workgroup witnesses, so this helper
/// accepts that shape only when one workgroup covers every writable element.
///
/// # Errors
///
/// Returns an actionable error when the program has a non-1D workgroup whose
/// writable footprint cannot be covered by a single workgroup.
pub fn config_for_program(program: &Program) -> Result<DispatchConfig, String> {
    let mut config = DispatchConfig::default();
    let tolerance = f32_ulp_tolerance(program);
    if tolerance > 0 {
        let tolerance = u8::try_from(tolerance).map_err(|_| {
            format!(
                "f32 ULP tolerance {tolerance} exceeds DispatchConfig::ulp_budget range. Fix: keep conform FP budgets <= u8::MAX."
            )
        })?;
        config.ulp_budget = Some(tolerance);
    }
    let workgroup = program.workgroup_size();
    for (axis, size) in workgroup.into_iter().enumerate() {
        if size == 0 {
            return Err(format!(
                "workgroup_size[{axis}] is 0. Fix: conform dispatch requires every workgroup dimension to be >= 1 before backend dispatch."
            ));
        }
    }
    if workgroup[1] == 1 && workgroup[2] == 1 {
        return Ok(config);
    }

    let lanes = u64::from(workgroup[0])
        .checked_mul(u64::from(workgroup[1]))
        .and_then(|lanes| lanes.checked_mul(u64::from(workgroup[2])))
        .ok_or_else(|| {
            format!(
                "workgroup_size {workgroup:?} overflows u64 lane accounting. Fix: use a valid backend workgroup shape."
            )
        })?;
    let max_writable_count = program
        .buffers()
        .iter()
        .filter(|decl| matches!(decl.access(), BufferAccess::ReadWrite) || decl.is_output())
        .map(|decl| u64::from(decl.count()))
        .max()
        .unwrap_or(1);

    if max_writable_count > lanes {
        return Err(format!(
            "non-1D workgroup_size {workgroup:?} has {lanes} lanes but the largest writable buffer has {max_writable_count} elements. Fix: register an explicit dispatch grid for this op instead of relying on the one-workgroup conform fixture path."
        ));
    }

    config.grid_override = Some([1, 1, 1]);
    Ok(config)
}
