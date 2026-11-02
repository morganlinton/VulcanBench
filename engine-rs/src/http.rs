//! The HTTP seam: a narrow client trait, the JSON POST helper that maps
//! status codes onto the retryable / final split of `harness/agent/providers.py`,
//! and the retry budget arithmetic (`_http_timeout`, `_budgeted_attempts`,
//! `_wait_exponential`, `_call_with_retry`).
//!
//! The trait is deliberately small (one `post`) so a rustls-based client could
//! replace the curl one without touching any provider.

use std::time::Duration;

use crate::json::{self, Value};
use crate::pyfmt::char_prefix;

/// Ceiling on a single provider HTTP call, independent of the run budget.
pub const DEFAULT_MAX_REQUEST_TIMEOUT_S: f64 = 600.0;

/// `_MAX_ATTEMPTS`: the full retry allowance when the run budget allows it.
pub const MAX_ATTEMPTS: u32 = 4;

/// 4xx codes worth another attempt: the server is asking us to wait, or the
/// conflict was momentary. Every other client error fails identically on retry.
const RETRYABLE_CLIENT_STATUS: [u16; 4] = [408, 409, 425, 429];

/// One POST with a JSON body. Headers carry the API key, so they must never
/// reach a process argument list or a file.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub timeout_s: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// A failure below the HTTP layer: no status code was produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The request hit the per-call timeout (curl exit 28, a socket timeout in Python).
    Timeout,
    /// Connection, DNS, TLS or protocol failure: `URLError` in Python, retryable.
    Transport(String),
    /// The client itself could not be used (binary missing, body too large): final.
    Setup(String),
}

pub trait HttpClient {
    /// # Errors
    ///
    /// [`HttpError`] when no HTTP status was obtained. A 4xx or 5xx response is
    /// an `Ok` response; [`post_json`] decides what it means.
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}

/// `ProviderError` with the `NonRetryableProviderError` distinction folded in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFailure {
    pub message: String,
    pub retryable: bool,
}

impl ProviderFailure {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
}

impl std::fmt::Display for ProviderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// `_http_post_json`: POST JSON, parse the JSON response, and classify failures.
///
/// # Errors
///
/// Retryable for transport failures, timeouts, 5xx and the few retryable 4xx
/// codes; final for every other 4xx and for a body that is not JSON.
pub fn post_json(
    client: &dyn HttpClient,
    url: &str,
    headers: &[(String, String)],
    payload: &Value,
    timeout_s: f64,
) -> Result<Value, ProviderFailure> {
    let request = HttpRequest {
        url: url.to_owned(),
        headers: headers.to_vec(),
        body: payload.to_json(),
        timeout_s,
    };
    let response = match client.post(&request) {
        Ok(response) => response,
        Err(HttpError::Timeout) => {
            return Err(ProviderFailure::retryable(format!(
                "read timeout after {timeout_s:.0}s calling {url}"
            )))
        }
        Err(HttpError::Transport(detail)) => {
            return Err(ProviderFailure::retryable(format!(
                "network error calling {url}: {detail}"
            )))
        }
        Err(HttpError::Setup(detail)) => {
            return Err(ProviderFailure::fatal(format!(
                "network error calling {url}: {detail}"
            )))
        }
    };
    if response.status >= 400 {
        let message = format!(
            "HTTP {} from {url}: {}",
            response.status,
            char_prefix(&response.body, 500)
        );
        let client_error = (400..500).contains(&response.status);
        if client_error && !RETRYABLE_CLIENT_STATUS.contains(&response.status) {
            return Err(ProviderFailure::fatal(message));
        }
        return Err(ProviderFailure::retryable(message));
    }
    json::parse(&response.body)
        .map_err(|e| ProviderFailure::fatal(format!("invalid JSON from {url}: {e}")))
}

/// `_http_timeout`: the per-request timeout is the request ceiling, capped by
/// the run's remaining budget.
///
/// # Errors
///
/// When the run budget is already exhausted.
pub fn http_timeout(timeout_s: Option<f64>, cap: f64) -> Result<f64, ProviderFailure> {
    match timeout_s {
        None => Ok(cap),
        Some(t) if t <= 0.0 => Err(ProviderFailure::retryable(
            "run budget exhausted before provider call",
        )),
        Some(t) => Ok(cap.min(t)),
    }
}

/// `_budgeted_attempts`: how many attempts fit in the remaining run budget.
pub fn budgeted_attempts(timeout_s: Option<f64>, request_timeout: f64) -> u32 {
    match timeout_s {
        None => MAX_ATTEMPTS,
        Some(_) if request_timeout <= 0.0 => 1,
        Some(t) => {
            let fit = (t / request_timeout).floor();
            let fit = if fit >= f64::from(MAX_ATTEMPTS) {
                MAX_ATTEMPTS
            } else {
                fit as u32
            };
            fit.clamp(1, MAX_ATTEMPTS)
        }
    }
}

/// `_wait_exponential`: seconds to sleep after failed attempt `attempt`
/// (1-based): 1, 2, 4, 8, capped at 20.
pub fn wait_exponential(attempt: u32) -> f64 {
    2.0_f64
        .powi(attempt.saturating_sub(1) as i32)
        .clamp(1.0, 20.0)
}

/// `_call_with_retry`: retry retryable failures up to `attempts` times,
/// sleeping the exponential schedule in between; final failures surface at once.
///
/// # Errors
///
/// The last failure when every attempt failed, or the first final failure.
pub fn call_with_retry<T>(
    attempts: u32,
    sleep: &dyn Fn(Duration),
    mut attempt_fn: impl FnMut() -> Result<T, ProviderFailure>,
) -> Result<T, ProviderFailure> {
    let attempts = attempts.max(1);
    let mut attempt = 1;
    loop {
        match attempt_fn() {
            Ok(value) => return Ok(value),
            Err(failure) if !failure.retryable || attempt >= attempts => return Err(failure),
            Err(_) => {
                sleep(Duration::from_secs_f64(wait_exponential(attempt)));
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Canned(RefCell<Vec<Result<HttpResponse, HttpError>>>);

    impl HttpClient for Canned {
        fn post(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpError> {
            self.0.borrow_mut().remove(0)
        }
    }

    fn response(status: u16, body: &str) -> Result<HttpResponse, HttpError> {
        Ok(HttpResponse {
            status,
            body: body.to_owned(),
        })
    }

    #[test]
    fn status_split_matches_python() {
        let client = Canned(RefCell::new(vec![
            response(200, r#"{"ok": true}"#),
            response(429, "slow down"),
            response(400, "bad"),
            response(503, "down"),
            response(409, "conflict"),
            Err(HttpError::Timeout),
            Err(HttpError::Transport("curl exit 7".to_owned())),
            Err(HttpError::Setup("no curl".to_owned())),
            response(200, "not json"),
        ]));
        let payload = Value::object();
        let call = || post_json(&client, "https://x/v1", &[], &payload, 12.0);
        assert_eq!(call().expect("ok").get("ok"), Some(&Value::Bool(true)));
        let too_many = call().expect_err("429");
        assert!(too_many.retryable);
        assert_eq!(too_many.message, "HTTP 429 from https://x/v1: slow down");
        assert!(!call().expect_err("400").retryable);
        assert!(call().expect_err("503").retryable);
        assert!(call().expect_err("409").retryable);
        let timeout = call().expect_err("timeout");
        assert_eq!(
            timeout.message,
            "read timeout after 12s calling https://x/v1"
        );
        assert!(timeout.retryable);
        let transport = call().expect_err("transport");
        assert_eq!(
            transport.message,
            "network error calling https://x/v1: curl exit 7"
        );
        assert!(transport.retryable);
        assert!(!call().expect_err("setup").retryable);
        assert!(!call().expect_err("json").retryable);
    }

    #[test]
    fn budget_arithmetic_matches_python() {
        assert_eq!(http_timeout(None, 600.0).expect("cap"), 600.0);
        assert_eq!(http_timeout(Some(42.5), 600.0).expect("budget"), 42.5);
        assert_eq!(http_timeout(Some(900.0), 600.0).expect("cap wins"), 600.0);
        assert!(http_timeout(Some(0.0), 600.0).is_err());
        assert_eq!(budgeted_attempts(None, 600.0), 4);
        assert_eq!(budgeted_attempts(Some(1500.0), 600.0), 2);
        assert_eq!(budgeted_attempts(Some(100.0), 600.0), 1);
        assert_eq!(budgeted_attempts(Some(10_000.0), 600.0), 4);
        assert_eq!(budgeted_attempts(Some(10.0), 0.0), 1);
        let schedule: Vec<f64> = (1..=6).map(wait_exponential).collect();
        assert_eq!(schedule, [1.0, 2.0, 4.0, 8.0, 16.0, 20.0]);
    }

    #[test]
    fn retry_loop_sleeps_between_retryable_failures_only() {
        let slept = RefCell::new(Vec::new());
        let sleep = |d: Duration| slept.borrow_mut().push(d.as_secs_f64());
        let mut calls = 0;
        let result = call_with_retry(4, &sleep, || {
            calls += 1;
            if calls < 3 {
                Err(ProviderFailure::retryable("stall"))
            } else {
                Ok(calls)
            }
        });
        assert_eq!(result.expect("third try"), 3);
        assert_eq!(*slept.borrow(), vec![1.0, 2.0]);

        slept.borrow_mut().clear();
        let mut calls = 0;
        let fatal = call_with_retry(4, &sleep, || {
            calls += 1;
            Err::<(), _>(ProviderFailure::fatal("refused"))
        });
        assert_eq!(fatal.expect_err("fatal").message, "refused");
        assert_eq!(calls, 1);
        assert!(slept.borrow().is_empty());

        let mut calls = 0;
        let exhausted = call_with_retry(2, &sleep, || {
            calls += 1;
            Err::<(), _>(ProviderFailure::retryable("stall"))
        });
        assert!(exhausted.is_err());
        assert_eq!(calls, 2);
        assert_eq!(*slept.borrow(), vec![1.0]);
    }
}
