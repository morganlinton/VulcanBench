use std::time::Instant;

pub(crate) fn push_stage<F>(
    stages: &mut Vec<crate::quick::quick_stage::QuickStage>,
    name: &'static str,
    f: F,
) where
    F: FnOnce() -> (crate::quick::quick_status::QuickStatus, String),
{
    let start = Instant::now();
    let (status, detail) = f();
    stages.push(crate::quick::quick_stage::QuickStage {
        name,
        status,
        duration: start.elapsed(),
        detail,
    });
}
