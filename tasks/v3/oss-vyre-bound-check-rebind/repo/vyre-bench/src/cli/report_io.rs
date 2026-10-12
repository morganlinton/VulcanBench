use crate::report::json::ReportSchema;

const MAX_REPORT_INPUT_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn load_report(path: &str) -> anyhow::Result<ReportSchema> {
    let bytes = read_report_bounded(std::path::Path::new(path))?;
    parse_report(&bytes, path)
}

pub(super) fn parse_report(bytes: &[u8], path: &str) -> anyhow::Result<ReportSchema> {
    let report: ReportSchema = serde_json::from_slice(bytes)?;
    report
        .validate_summary_evidence()
        .map_err(|error| anyhow::anyhow!("invalid benchmark report `{}`: {error}", path))?;
    report
        .validate_blocker_evidence()
        .map_err(|error| anyhow::anyhow!("invalid benchmark report `{}`: {error}", path))?;
    Ok(report)
}

pub(super) fn read_report_bounded(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_REPORT_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("benchmark report exceeds {MAX_REPORT_INPUT_BYTES} byte limit"),
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_REPORT_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_REPORT_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "benchmark report exceeded bounded read limit",
        ));
    }
    Ok(bytes)
}

pub(super) fn validate_report_expectations(
    report: &ReportSchema,
    backend: Option<&str>,
    total_cases: Option<usize>,
    failed: Option<usize>,
) -> anyhow::Result<()> {
    report
        .validate_backend_profile_evidence(backend)
        .map_err(|error| anyhow::anyhow!("invalid benchmark report backend profile: {error}"))?;
    report
        .validate_benchmark_case_evidence_schema()
        .map_err(|error| anyhow::anyhow!("invalid benchmark report case evidence: {error}"))?;
    if let Some(expected_backend) = backend {
        if report.selected_backend.as_deref() != Some(expected_backend) {
            anyhow::bail!(
                "selected_backend {:?} does not match expected backend `{expected_backend}`. Fix: rerun the benchmark with --backend {expected_backend}.",
                report.selected_backend
            );
        }
    }
    if let Some(total_cases) = total_cases {
        if report.summary.total_cases != total_cases {
            anyhow::bail!(
                "summary.total_cases={} does not match expected total_cases={total_cases}. Fix: rerun the benchmark with the intended --case selection.",
                report.summary.total_cases
            );
        }
    }
    if let Some(failed) = failed {
        if report.summary.failed != failed {
            anyhow::bail!(
                "summary.failed={} does not match expected failed={failed}. Fix: inspect blockers and rerun after fixing failing benchmark cases.",
                report.summary.failed
            );
        }
    }
    Ok(())
}
