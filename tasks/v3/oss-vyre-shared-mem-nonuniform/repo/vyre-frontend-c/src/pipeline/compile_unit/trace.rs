pub(super) struct CompileTrace {
    enabled: bool,
    started: std::time::Instant,
    last: std::time::Instant,
}

impl CompileTrace {
    pub(super) fn new() -> Self {
        let started = std::time::Instant::now();
        Self {
            enabled: std::env::var("VYRE_STAGE_TRACE").is_ok(),
            started,
            last: started,
        }
    }

    pub(super) fn log(&mut self, label: &str) {
        if !self.enabled {
            return;
        }
        let now = std::time::Instant::now();
        let stage = now.duration_since(self.last).as_millis();
        let total = now.duration_since(self.started).as_millis();
        eprintln!("[stage-trace] +{stage}ms (total {total}ms): {label}");
        self.last = now;
    }
}
