use std::path::Path;
use std::time::Instant;

pub(super) struct StageTrace<'a> {
    enabled: bool,
    stage_start: Instant,
    last_t: Instant,
    depth: u32,
    file_path: &'a Path,
    source_len: usize,
}

impl<'a> StageTrace<'a> {
    pub(super) fn new(depth: u32, file_path: &'a Path, source_len: usize) -> Self {
        let now = Instant::now();
        Self {
            enabled: std::env::var_os("VYRE_STAGE_TRACE").is_some(),
            stage_start: now,
            last_t: now,
            depth,
            file_path,
            source_len,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn log(&mut self, label: &str) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        let stage = now.duration_since(self.last_t).as_micros();
        let total = now.duration_since(self.stage_start).as_micros();
        tracing::debug!(
            "[stage-trace] +{stage}us (total {total}us): gpu-preprocess depth={} bytes={} {} {label}",
            self.depth,
            self.source_len,
            self.file_path.display()
        );
        self.last_t = now;
    }
}
