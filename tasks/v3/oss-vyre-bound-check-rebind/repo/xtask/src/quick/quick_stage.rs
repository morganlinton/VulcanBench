use std::time::Duration;

pub(crate) struct QuickStage {
    pub(crate) name: &'static str,
    pub(crate) status: crate::quick::quick_status::QuickStatus,
    pub(crate) duration: Duration,
    pub(crate) detail: String,
}
