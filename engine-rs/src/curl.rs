//! A curl-backed [`HttpClient`]: one `curl --config -` process per request.
//!
//! The URL, every header (including the API key) and the request body travel
//! to curl as a config file on stdin, so nothing sensitive appears in the
//! process argument list, in the environment or in a file. curl owns TLS,
//! proxies and HTTP/2; the engine owns the retry policy and the status
//! classification ([`crate::http::post_json`]).
//!
//! The HTTP status comes back on stdout after the body, behind a marker set
//! with `--write-out`, so a single pipe carries both.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::http::{HttpClient, HttpError, HttpRequest, HttpResponse};
use crate::process;

/// Separates the response body from the status code on curl's stdout.
const STATUS_MARKER: &str = "\n__VULCANBENCH_HTTP_STATUS__";

/// curl parses a config file line by line with a 10 MiB line limit, and the
/// body is one line, so refuse anything close to it with a clear message.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Extra time allowed for curl to exit after its own `--max-time` fires.
const EXIT_GRACE_S: f64 = 30.0;

/// Exit status curl uses for an operation timeout.
const CURL_EXIT_TIMEOUT: i32 = 28;

#[derive(Debug, Clone)]
pub struct CurlClient {
    binary: PathBuf,
}

impl CurlClient {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// The `curl` on `PATH`, or `VULCANBENCH_CURL_BIN` when set. A missing
    /// binary is reported at request time, like a missing network.
    pub fn from_environment() -> Self {
        let name = std::env::var("VULCANBENCH_CURL_BIN").unwrap_or_else(|_| "curl".to_owned());
        Self::new(process::which(&name).unwrap_or_else(|| PathBuf::from(name)))
    }

    /// Quotes a value for a curl config line: double quotes with backslash
    /// escapes for the characters curl's parser treats specially.
    pub fn quote(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        for ch in value.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{b}' => out.push_str("\\v"),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// The config file text for one request (what curl reads from stdin).
    pub fn config_text(request: &HttpRequest) -> String {
        let mut text = String::new();
        text.push_str(&format!("url = {}\n", Self::quote(&request.url)));
        text.push_str("request = \"POST\"\n");
        text.push_str("silent\n");
        text.push_str("show-error\n");
        // urllib never sends Expect: 100-continue; keep the wire shape the same.
        text.push_str("header = \"Expect:\"\n");
        for (name, value) in &request.headers {
            text.push_str(&format!(
                "header = {}\n",
                Self::quote(&format!("{name}: {value}"))
            ));
        }
        text.push_str(&format!("data-binary = {}\n", Self::quote(&request.body)));
        text.push_str(&format!("max-time = {}\n", request.timeout_s));
        text.push_str(&format!(
            "write-out = {}\n",
            Self::quote(&format!("{STATUS_MARKER}%{{http_code}}"))
        ));
        text
    }

    fn split_status(stdout: &str) -> Option<(u16, String)> {
        let idx = stdout.rfind(STATUS_MARKER)?;
        let status = stdout[idx + STATUS_MARKER.len()..].trim().parse().ok()?;
        Some((status, stdout[..idx].to_owned()))
    }
}

impl HttpClient for CurlClient {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        if request.body.len() > MAX_BODY_BYTES {
            return Err(HttpError::Setup(format!(
                "request body of {} bytes exceeds the {MAX_BODY_BYTES}-byte curl config limit",
                request.body.len()
            )));
        }
        let config = Self::config_text(request);
        let mut command = Command::new(&self.binary);
        command.args(["--config", "-"]);
        let timeout = Duration::from_secs_f64(request.timeout_s.max(0.0) + EXIT_GRACE_S);
        let done = process::run_with_input(&mut command, Some(config.into_bytes()), Some(timeout))
            .map_err(|e| HttpError::Setup(format!("cannot run {}: {e}", self.binary.display())))?;
        if done.timed_out || done.exit_code == CURL_EXIT_TIMEOUT {
            return Err(HttpError::Timeout);
        }
        if done.exit_code != 0 {
            let detail = process::first_output_line(&done.stderr).unwrap_or_default();
            return Err(HttpError::Transport(format!(
                "curl exit {}: {}",
                done.exit_code,
                if detail.is_empty() {
                    "no detail"
                } else {
                    detail.as_str()
                }
            )));
        }
        match Self::split_status(&done.stdout) {
            Some((status, body)) => Ok(HttpResponse { status, body }),
            None => Err(HttpError::Transport(
                "curl exited 0 without reporting an HTTP status".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::post_json;
    use crate::json::{obj, Value};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// A stand-in curl: records argv and the stdin config beside itself, then
    /// prints a canned body, the status marker and exits with a chosen code.
    fn fake_curl(dir: &Path, body: &str, status: &str, exit: i32) -> PathBuf {
        fs::create_dir_all(dir).expect("dir");
        let script = dir.join("curl");
        fs::write(dir.join("curl.body"), body).expect("body");
        fs::write(dir.join("curl.status"), status).expect("status");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s' \"$*\" > \"$0.args\"\ncat > \"$0.config\"\ncat \"$0.body\"\n\
                 printf '\\n__VULCANBENCH_HTTP_STATUS__%s' \"$(cat \"$0.status\")\"\nexit {exit}\n"
            ),
        )
        .expect("script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
        script
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vb-curl-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn request() -> HttpRequest {
        HttpRequest {
            url: "https://api.example.test/v1/messages".to_owned(),
            headers: vec![
                (
                    "x-api-key".to_owned(),
                    "sk-ant-secret-value-0123456789".to_owned(),
                ),
                ("Content-Type".to_owned(), "application/json".to_owned()),
            ],
            body: r#"{"model": "m", "text": "quote \" and backslash \\"}"#.to_owned(),
            timeout_s: 42.0,
        }
    }

    #[test]
    fn secrets_travel_on_stdin_not_argv() {
        let dir = scratch("argv");
        let script = fake_curl(&dir, r#"{"id": "msg"}"#, "200", 0);
        let client = CurlClient::new(&script);
        let response = client.post(&request()).expect("ok");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, r#"{"id": "msg"}"#);
        let args = fs::read_to_string(dir.join("curl.args")).expect("args");
        assert_eq!(args, "--config -");
        let config = fs::read_to_string(dir.join("curl.config")).expect("config");
        assert!(config.contains("url = \"https://api.example.test/v1/messages\"\n"));
        assert!(config.contains("header = \"x-api-key: sk-ant-secret-value-0123456789\"\n"));
        assert!(config.contains("request = \"POST\"\n"));
        assert!(config.contains("max-time = 42\n"));
        assert!(config.contains(
            "data-binary = \"{\\\"model\\\": \\\"m\\\", \\\"text\\\": \\\"quote \\\\\\\" and backslash \\\\\\\\\\\"}\"\n"
        ));
        assert!(config.contains("write-out = \"\\n__VULCANBENCH_HTTP_STATUS__%{http_code}\"\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exit_codes_and_statuses_map_onto_the_python_split() {
        let dir = scratch("map");
        let payload = obj! { "k" => "v" };
        let url = "https://api.example.test/v1";

        fake_curl(&dir, "", "000", 28);
        let client = CurlClient::new(dir.join("curl"));
        assert_eq!(client.post(&request()), Err(HttpError::Timeout));
        let failure = post_json(&client, url, &[], &payload, 9.0).expect_err("timeout");
        assert!(failure.retryable);
        assert_eq!(
            failure.message,
            format!("read timeout after 9s calling {url}")
        );

        fake_curl(&dir, "", "000", 7);
        let failure = post_json(&client, url, &[], &payload, 9.0).expect_err("connect");
        assert!(failure.retryable);
        assert!(failure
            .message
            .starts_with(&format!("network error calling {url}: curl exit 7")));

        fake_curl(&dir, r#"{"error": "rate"}"#, "429", 0);
        assert!(
            post_json(&client, url, &[], &payload, 9.0)
                .expect_err("429")
                .retryable
        );

        fake_curl(&dir, r#"{"error": "bad key"}"#, "401", 0);
        let unauthorized = post_json(&client, url, &[], &payload, 9.0).expect_err("401");
        assert!(!unauthorized.retryable);
        assert_eq!(
            unauthorized.message,
            format!("HTTP 401 from {url}: {{\"error\": \"bad key\"}}")
        );

        fake_curl(&dir, "", "502", 0);
        assert!(
            post_json(&client, url, &[], &payload, 9.0)
                .expect_err("502")
                .retryable
        );

        fake_curl(&dir, r#"{"content": []}"#, "200", 0);
        let body = post_json(&client, url, &[], &payload, 9.0).expect("ok");
        assert_eq!(body.get("content"), Some(&Value::Array(Vec::new())));

        let missing = CurlClient::new(dir.join("no-such-curl"));
        assert!(matches!(missing.post(&request()), Err(HttpError::Setup(_))));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn quoting_round_trips_special_characters() {
        assert_eq!(
            CurlClient::quote("a\"b\\c\nd\te"),
            "\"a\\\"b\\\\c\\nd\\te\""
        );
        assert_eq!(CurlClient::quote("plain"), "\"plain\"");
        let oversized = HttpRequest {
            body: "x".repeat(MAX_BODY_BYTES + 1),
            ..request()
        };
        let client = CurlClient::new("/bin/true");
        assert!(matches!(client.post(&oversized), Err(HttpError::Setup(_))));
    }
}
