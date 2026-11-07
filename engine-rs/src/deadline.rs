//! The run's wall-clock budget (`_RunDeadline` in `harness/agent/loop.py`).

use std::time::Instant;

use crate::error::EngineError;
use crate::json::obj;
use crate::tracer::TraceCollector;

#[derive(Debug)]
pub struct Deadline {
    started: Instant,
    timeout_s: Option<f64>,
    pub exceeded: bool,
}

impl Deadline {
    pub fn new(timeout_s: Option<f64>) -> Self {
        Self {
            started: Instant::now(),
            timeout_s,
            exceeded: false,
        }
    }

    pub fn elapsed_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64().max(0.0)
    }

    /// Seconds left, or `None` for an unbounded run.
    pub fn remaining_s(&self) -> Option<f64> {
        self.timeout_s
            .map(|timeout| (timeout - self.elapsed_s()).max(0.0))
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_s().is_some_and(|remaining| remaining <= 0.0)
    }

    /// A subprocess timeout: `default`, capped by what is left; `None` once expired.
    pub fn timeout_for(&self, default: i64) -> Option<i64> {
        clamp_timeout(default, self.remaining_s())
    }

    /// `True` while time remains; otherwise records `budget_exceeded` once.
    ///
    /// # Errors
    ///
    /// When the trace cannot be written.
    pub fn ensure_time(
        &mut self,
        collector: &mut TraceCollector,
        stage: &str,
        step: Option<i64>,
    ) -> Result<bool, EngineError> {
        if !self.is_expired() {
            return Ok(true);
        }
        self.record_exceeded(collector, stage, step)?;
        Ok(false)
    }

    /// Records the first budget overrun with the stage it happened in.
    ///
    /// # Errors
    ///
    /// When the trace cannot be written.
    pub fn record_exceeded(
        &mut self,
        collector: &mut TraceCollector,
        stage: &str,
        step: Option<i64>,
    ) -> Result<(), EngineError> {
        if self.exceeded {
            return Ok(());
        }
        self.exceeded = true;
        let mut payload = obj! {
            "timeout_s" => self.timeout_s,
            "elapsed_s" => crate::pyfmt::py_round(self.elapsed_s(), 3),
            "stage" => stage,
        };
        if let Some(step) = step {
            payload.insert("step", step);
        }
        collector.record("budget_exceeded", payload)
    }
}

/// `_remaining_timeout` / `_timeout`: `default` when unbounded, `None` when the
/// budget is gone, otherwise `max(1, min(default, ceil(remaining)))`.
pub fn clamp_timeout(default: i64, remaining: Option<f64>) -> Option<i64> {
    match remaining {
        None => Some(default),
        Some(r) if r <= 0.0 => None,
        Some(r) => Some(default.min(r.ceil() as i64).max(1)),
    }
}

/// `_tool_timeout`: at most 120 seconds, at least 1, `None` when unbounded.
pub fn tool_timeout(remaining: Option<f64>) -> Option<i64> {
    remaining.map(|r| 120.min(r.ceil() as i64).max(1))
}

pub fn budget_exhausted(remaining: Option<f64>) -> bool {
    remaining.is_some_and(|r| r <= 0.0)
}

impl Default for Deadline {
    fn default() -> Self {
        Self::new(None)
    }
}
