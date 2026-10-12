use crate::api::suite::SuiteKind;
use crate::report::json::ReportSchema;
use crate::runner::{execute_suite, RunConfig};

pub(super) fn execute_run_matrix(
    registry: &crate::registry::BenchRegistry,
    suite: SuiteKind,
    config: &RunConfig,
) -> anyhow::Result<Vec<ReportSchema>> {
    match suite {
        SuiteKind::CrossBackend if config.backend_id.is_none() => {
            let mut reports = Vec::new();
            for backend in dispatch_backend_ids() {
                let mut cfg = config.clone();
                cfg.backend_id = Some(backend.to_string());
                reports.push(execute_suite(registry, suite, &cfg));
            }
            Ok(reports)
        }
        SuiteKind::Sweep if config.workgroup_override.is_none() => {
            let mut reports = Vec::new();
            for size in [32, 64, 128, 256] {
                let mut cfg = config.clone();
                cfg.workgroup_override = Some([size, 1, 1]);
                reports.push(execute_suite(registry, suite, &cfg));
            }
            Ok(reports)
        }
        _ => Ok(vec![execute_suite(registry, suite, config)]),
    }
}

fn dispatch_backend_ids() -> Vec<&'static str> {
    vyre_driver::backend::registered_backends_by_precedence_slice()
        .iter()
        .filter(|backend| vyre_driver::backend::backend_dispatches(backend.id))
        .map(|backend| backend.id)
        .collect()
}

pub(super) fn write_run_reports(reports: &[ReportSchema], output: &str) -> anyhow::Result<()> {
    let output = std::path::Path::new(output);
    if reports.len() == 1 {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            output,
            format!(
                "{}\n",
                crate::report::json::generate_json_report(&reports[0])?
            ),
        )?;
        return Ok(());
    }
    std::fs::create_dir_all(output)?;
    for (index, report) in reports.iter().enumerate() {
        let suite = sanitize_path_component(&report.suite);
        let backend = report
            .selected_backend
            .as_deref()
            .map(sanitize_path_component)
            .unwrap_or_else(|| "unknown-backend".to_string());
        let path = output.join(format!("{suite}-{backend}-{index:03}.json"));
        std::fs::write(
            path,
            format!("{}\n", crate::report::json::generate_json_report(report)?),
        )?;
    }
    Ok(())
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
