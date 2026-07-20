use crate::api::case::{prepared_program, BenchContext, BenchError, BenchRun, PreparedCase};
use crate::api::metric::{BenchMetrics, MetricPoint};

pub(crate) fn gpu_case_metrics(
    wall_ns: u64,
    dispatch_ns: Option<u64>,
    input_bytes: u64,
    output_bytes: u64,
    work_units: u64,
) -> BenchMetrics {
    BenchMetrics {
        wall_ns: Some(wall_ns),
        dispatch_ns,
        input_bytes: Some(input_bytes),
        output_bytes: Some(output_bytes),
        custom: vec![MetricPoint {
            name: "flop_count".to_string(),
            value: work_units,
        }],
        ..Default::default()
    }
}

pub(crate) fn run_gpu_with_cpu_baseline<F>(
    ctx: &mut BenchContext,
    prepared: &mut PreparedCase,
    inputs: Vec<Vec<u8>>,
    work_units: u64,
    baseline: F,
) -> Result<BenchRun, BenchError>
where
    F: FnOnce(&[Vec<u8>]) -> Vec<Vec<u8>>,
{
    let prog = prepared_program(prepared)?;
    let input_bytes = inputs.iter().map(Vec::len).sum::<usize>() as u64;

    let timed = ctx
        .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
    let wall_ns = timed.wall_ns;
    let dispatch_ns = timed.device_ns;
    let outputs = timed.outputs;
    let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;

    let start_ref = std::time::Instant::now();
    let baseline_outputs = baseline(&inputs);
    let baseline_wall_ns = start_ref.elapsed().as_nanos() as u64;
    let baseline_output_bytes = baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64;

    Ok(BenchRun {
        metrics: gpu_case_metrics(wall_ns, dispatch_ns, input_bytes, output_bytes, work_units),
        baseline_metrics: Some(gpu_case_metrics(
            baseline_wall_ns,
            None,
            input_bytes,
            baseline_output_bytes,
            work_units,
        )),
        outputs,
        baseline_outputs: Some(baseline_outputs),
    })
}

#[cfg(test)]
mod tests {
    use super::gpu_case_metrics;

    #[test]
    fn generated_gpu_case_metrics_preserve_io_and_work_units() {
        let mut checked = 0_u32;
        for lanes in 0_u64..=2_048 {
            let input_bytes = lanes.saturating_mul(4);
            let output_bytes = lanes.saturating_mul(8);
            let work_units = lanes.saturating_mul(3);
            let metrics = gpu_case_metrics(
                11 + lanes,
                Some(7 + lanes),
                input_bytes,
                output_bytes,
                work_units,
            );

            assert_eq!(metrics.wall_ns, Some(11 + lanes));
            assert_eq!(metrics.dispatch_ns, Some(7 + lanes));
            assert_eq!(metrics.input_bytes, Some(input_bytes));
            assert_eq!(metrics.output_bytes, Some(output_bytes));
            assert_eq!(metrics.custom.len(), 1);
            assert_eq!(metrics.custom[0].name, "flop_count");
            assert_eq!(metrics.custom[0].value, work_units);
            checked += 1;
        }
        assert_eq!(checked, 2_049);
    }
}
