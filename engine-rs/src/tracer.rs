//! The trace collector (`harness/tracer/collector.py`): one sanitized JSON
//! line per event in `trace.jsonl`, and `summary.json` at the end.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::EngineError;
use crate::json::{obj, Value};
use crate::pyfmt::utc_now_isoformat;
use crate::redaction::sanitize;

#[derive(Debug)]
pub struct TraceCollector {
    pub run_dir: PathBuf,
    pub trace_path: PathBuf,
    pub summary_path: PathBuf,
    pub run_id: String,
    pub task_id: String,
    pub model: String,
    pub step: i64,
}

impl TraceCollector {
    /// Creates the run directory and an empty collector.
    ///
    /// # Errors
    ///
    /// When the run directory cannot be created.
    pub fn new(run_dir: &Path, run_id: &str, task_id: &str, model: &str) -> Result<Self, EngineError> {
        fs::create_dir_all(run_dir)
            .map_err(|e| EngineError::io(format!("creating {}", run_dir.display()), e))?;
        Ok(Self {
            run_dir: run_dir.to_path_buf(),
            trace_path: run_dir.join("trace.jsonl"),
            summary_path: run_dir.join("summary.json"),
            run_id: run_id.to_owned(),
            task_id: task_id.to_owned(),
            model: model.to_owned(),
            step: 0,
        })
    }

    /// Appends one event. Secrets are redacted and fields capped before the
    /// line touches disk.
    ///
    /// # Errors
    ///
    /// When the trace file cannot be appended to.
    pub fn record(&mut self, event_type: &str, data: Value) -> Result<(), EngineError> {
        self.step += 1;
        let event = obj! {
            "ts" => utc_now_isoformat(),
            "step" => self.step,
            "run_id" => self.run_id.as_str(),
            "task_id" => self.task_id.as_str(),
            "model" => self.model.as_str(),
            "type" => event_type,
            "data" => sanitize(&data),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.trace_path)
            .map_err(|e| EngineError::io(format!("opening {}", self.trace_path.display()), e))?;
        writeln!(file, "{}", event.to_json())
            .map_err(|e| EngineError::io("writing trace event", e))
    }

    /// Writes `summary.json` and returns the sanitized summary.
    ///
    /// # Errors
    ///
    /// When the summary cannot be written.
    pub fn finalize(&self, scores: Value, extra: Value) -> Result<Value, EngineError> {
        let mut summary = obj! {
            "run_id" => self.run_id.as_str(),
            "task_id" => self.task_id.as_str(),
            "model" => self.model.as_str(),
            "steps" => self.step,
            "scores" => scores,
            "artifacts" => obj! {
                "trace" => self.trace_path.to_string_lossy().into_owned(),
                "replay_html" => self.run_dir.join("replay.html").to_string_lossy().into_owned(),
            },
            "finished_at" => utc_now_isoformat(),
        };
        summary.extend(extra);
        let clean = sanitize(&summary);
        fs::write(&self.summary_path, clean.to_json_pretty())
            .map_err(|e| EngineError::io(format!("writing {}", self.summary_path.display()), e))?;
        Ok(clean)
    }
}
