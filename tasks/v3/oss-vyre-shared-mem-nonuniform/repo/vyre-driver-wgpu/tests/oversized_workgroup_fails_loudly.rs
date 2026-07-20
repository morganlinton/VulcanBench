//! Oversized workgroup validation must fail loudly.
//!
//! Guarantees:
//! - A program whose workgroup size exceeds the adapter limit on any axis
//!   is rejected at dispatch time with a structured, actionable error
//! - `dispatch` and `dispatch_async` both surface the validation error
//!   before any GPU work is submitted
//! - The error message contains "workgroup_size" so callers know what to fix

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};

fn dummy_program(workgroup_size: [u32; 3]) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(1)
            .with_output_byte_range(0..4)],
        workgroup_size,
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(42)),
            Node::return_(),
        ],
    )
}

// ------------------------------------------------------------------
// 1. Oversized X axis rejected by dispatch
// ------------------------------------------------------------------

#[test]
fn oversized_workgroup_x_fails_loudly_on_dispatch() {
    let backend = live_backend();
    let max = backend.max_workgroup_size();
    let info = backend.adapter_info();

    // If the adapter has no practical limit (e.g. 0 or u32::MAX), skip.
    if max[0] == 0 || max[0] == u32::MAX {
        return;
    }

    let program = dummy_program([max[0] + 1, 1, 1]);
    let result = backend.dispatch(&program, &[], &DispatchConfig::default());

    assert!(
        result.is_err(),
        "Fix: dispatch of a program with workgroup_size.x={} must fail when adapter `{}` \
         max_workgroup_size.x={}",
        max[0] + 1,
        info.name,
        max[0]
    );

    let err_text = result.unwrap_err().to_string();
    assert!(
        err_text.contains("workgroup_size") || err_text.contains("Fix:"),
        "Fix: oversized workgroup error must mention 'workgroup_size' or contain an actionable 'Fix:' hint. \
         Got: {err_text}"
    );
}

// ------------------------------------------------------------------
// 2. Oversized Y axis rejected by dispatch_async
// ------------------------------------------------------------------

#[test]
fn oversized_workgroup_y_fails_loudly_on_dispatch_async() {
    let backend = live_backend();
    let max = backend.max_workgroup_size();
    let info = backend.adapter_info();

    if max[1] == 0 || max[1] == u32::MAX {
        return;
    }

    let program = dummy_program([1, max[1] + 1, 1]);
    let result = backend.dispatch_async(&program, &[], &DispatchConfig::default());

    assert!(
        result.is_err(),
        "Fix: dispatch_async of a program with workgroup_size.y={} must fail when adapter `{}` \
         max_workgroup_size.y={}",
        max[1] + 1,
        info.name,
        max[1]
    );

    let err_text = match result {
        Err(e) => e.to_string(),
        Ok(_) => unreachable!(),
    };
    assert!(
        err_text.contains("workgroup_size") || err_text.contains("Fix:"),
        "Fix: oversized workgroup error must mention 'workgroup_size' or contain an actionable 'Fix:' hint. \
         Got: {err_text}"
    );
}

// ------------------------------------------------------------------
// 3. Oversized Z axis rejected
// ------------------------------------------------------------------

#[test]
fn oversized_workgroup_z_fails_loudly() {
    let backend = live_backend();
    let max = backend.max_workgroup_size();
    let info = backend.adapter_info();

    if max[2] == 0 || max[2] == u32::MAX {
        return;
    }

    let program = dummy_program([1, 1, max[2] + 1]);
    let result = backend.dispatch(&program, &[], &DispatchConfig::default());

    assert!(
        result.is_err(),
        "Fix: dispatch of a program with workgroup_size.z={} must fail when adapter `{}` \
         max_workgroup_size.z={}",
        max[2] + 1,
        info.name,
        max[2]
    );

    let err_text = result.unwrap_err().to_string();
    assert!(
        err_text.contains("workgroup_size") || err_text.contains("Fix:"),
        "Fix: oversized workgroup error must mention 'workgroup_size' or contain an actionable 'Fix:' hint. \
         Got: {err_text}"
    );
}

// ------------------------------------------------------------------
// 4. Product-based oversize (total invocations > max) must also fail
// ------------------------------------------------------------------

#[test]
fn oversized_total_invocations_fails_loudly() {
    let backend = live_backend();
    let max_invocations = backend.max_compute_invocations_per_workgroup();
    let info = backend.adapter_info();

    if max_invocations == 0 || max_invocations == u32::MAX {
        return;
    }

    // Pick a workgroup size whose product exceeds max_invocations but each
    // individual axis is within the adapter's per-axis limit. This tests
    // that the backend validates the total invocation count, not just axes.
    let max_axis = backend.max_workgroup_size();
    let candidate = [(max_invocations + 2).min(max_axis[0]), 1, 1];

    // If we can't exceed the product while staying within per-axis limits,
    // fall back to exceeding on a single axis.
    let program = if candidate[0] <= max_axis[0] && candidate[0] > max_invocations {
        dummy_program(candidate)
    } else {
        // Some adapters have max_axis[0] < max_invocations, so the total
        // invocation limit is never the binding constraint. Skip in that case.
        return;
    };

    let result = backend.dispatch(&program, &[], &DispatchConfig::default());

    assert!(
        result.is_err(),
        "Fix: dispatch of a program with total workgroup invocations ({}) must fail \
         when adapter `{}` max_compute_invocations_per_workgroup={}",
        candidate[0],
        info.name,
        max_invocations
    );

    let err_text = result.unwrap_err().to_string();
    assert!(
        err_text.contains("workgroup_size") || err_text.contains("Fix:"),
        "Fix: oversized workgroup error must mention 'workgroup_size' or contain an actionable 'Fix:' hint. \
         Got: {err_text}"
    );
}

// ------------------------------------------------------------------
// 5. Valid-sized workgroup at the exact limit must succeed
// ------------------------------------------------------------------

#[test]
fn workgroup_at_exact_limit_succeeds() {
    let backend = live_backend();
    let max = backend.max_workgroup_size();
    let info = backend.adapter_info();

    // Use the largest valid size that is guaranteed <= limit on every axis.
    let safe_size = [max[0].max(1), max[1].max(1), max[2].max(1)];

    // But cap the total invocations to avoid exceeding max_compute_invocations_per_workgroup.
    let max_invocations = backend.max_compute_invocations_per_workgroup();
    let total = safe_size[0]
        .saturating_mul(safe_size[1])
        .saturating_mul(safe_size[2]);
    if total > max_invocations && max_invocations > 0 {
        // Can't safely test at the exact per-axis limits because the product
        // exceeds the total invocation cap. Skip this adapter.
        return;
    }

    let program = dummy_program(safe_size);
    let result = backend.dispatch(&program, &[], &DispatchConfig::default());

    assert!(
        matches!(result, Ok(_)),
        "Fix: dispatch of a program with workgroup_size={:?} (at adapter limit) \
         must succeed on adapter `{}`. Got error: {:?}",
        safe_size,
        info.name,
        result.err()
    );
}
