use std::time::Duration;

pub(crate) struct QuickReport {
    pub(crate) op_id: String,
    pub(crate) stages: Vec<crate::quick::quick_stage::QuickStage>,
    pub(crate) total: Duration,
    pub(crate) pass: bool,
    pub(crate) reason: Option<String>,
}
