pub(crate) fn print_quick_report(report: &crate::quick::quick_report::QuickReport) {
    println!("quick-check report");
    println!("op: {}", report.op_id);
    for stage in &report.stages {
        println!(
            "stage {}: {} in {}ms - {}",
            stage.name,
            crate::quick::quick_status::QuickStatus::as_str(stage.status),
            stage.duration.as_millis(),
            stage.detail
        );
    }

    let seconds = crate::quick::format_seconds::format_seconds(report.total);
    if report.pass {
        println!("quick-check: PASS in {seconds}");
    } else {
        let fallback;
        let reason = if let Some(reason) = &report.reason {
            reason.as_str()
        } else {
            fallback = "unknown failure".to_string();
            fallback.as_str()
        };
        println!("quick-check: FAIL in {seconds} - {reason}");
        println!("quick-check: FAIL - {reason}");
    }
}
