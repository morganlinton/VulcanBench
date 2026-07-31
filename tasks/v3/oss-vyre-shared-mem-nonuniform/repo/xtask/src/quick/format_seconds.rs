use std::time::Duration;

pub(crate) fn format_seconds(duration: Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}
