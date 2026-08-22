//! Test runner emitted events.
//!
//! Required feature: `test_unstable` since the format parsed is unstable.

use serde::Deserialize;

// See https://github.com/rust-lang/rust/tree/master/src/libtest/formatters/json.rs

/// Test-runner event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum Event {
    /// Suite event.
    Suite(Suite),
    /// Test case event.
    Test(Test),
    /// Benchmark event.
    Bench(Bench),
    #[cfg(not(feature = "strict_unstable"))]
    #[doc(hidden)]
    #[serde(other)]
    Unknown,
}

/// Suite event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
pub enum Suite {
    /// Suite-started event.
    Started(SuiteStarted),
    /// Suite-finished successfully event.
    Ok(SuiteOk),
    /// Suite-finished with failure event.
    Failed(SuiteFailed),
    #[cfg(not(feature = "strict_unstable"))]
    #[doc(hidden)]
    #[serde(other)]
    Unknown,
}

/// Suite-started event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct SuiteStarted {
    /// Number of test cases in the suite.
    pub test_count: usize,
}

/// Suite-finished successfully event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct SuiteOk {
    /// Cases that passed.
    pub passed: usize,
    /// Cases that failed.
    pub failed: usize,
    /// Cases that were allowed to fail.
    pub allowed_fail: usize,
    /// Ignored cases.
    pub ignored: usize,
    /// Benchmarks
    pub measured: usize,
    /// Cases filtered out by caller.
    pub filtered_out: usize,
}

/// Suite-finished with failure event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct SuiteFailed {
    /// Cases that passed.
    pub passed: usize,
    /// Cases that failed.
    pub failed: usize,
    /// Cases that were allowed to fail.
    pub allowed_fail: usize,
    /// Ignored cases.
    pub ignored: usize,
    /// Benchmarks
    pub measured: usize,
    /// Cases filtered out by caller.
    pub filtered_out: usize,
}

/// Test case event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
pub enum Test {
    /// Case-started event.
    Started(TestStarted),
    /// Case-finished successfully event.
    Ok(TestOk),
    /// Case-finished with failure event.
    Failed(TestFailed),
    /// Case-ignored event.
    Ignored(TestIgnored),
    /// Case-allowed-failure event.
    AllowedFailure(TestAllowedFailure),
    /// Case-timeout event.
    Timeout(TestTimeout),
    #[cfg(not(feature = "strict_unstable"))]
    #[doc(hidden)]
    #[serde(other)]
    Unknown,
}

/// Case-started event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestStarted {
    /// Test case name.
    pub name: String,
}

/// Case-finished successfully event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestOk {
    /// Test case name.
    pub name: String,
}

/// Case-finished with failure event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestFailed {
    /// Test case name.
    pub name: String,
    /// Test's stdout
    pub stdout: Option<String>,
    /// Test failure mssage
    pub message: Option<String>,
}

/// Case-ignored event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestIgnored {
    /// Test case name.
    pub name: String,
}

/// Case-allowed-failure event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestAllowedFailure {
    /// Test case name.
    pub name: String,
}

/// Case-timeout event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TestTimeout {
    /// Test case name.
    pub name: String,
}

/// Benchmark event.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Bench {
    /// Benchmark name.
    pub name: String,
    /// Median performance.
    pub median: usize,
    /// Deviation from median.
    pub deviation: usize,
    /// Mb/s
    pub mib_per_second: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_started() {
        let input = r#"{ "type": "suite", "event": "started", "test_count": 10 }"#;
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn suite_ok() {
        let input = "{ \"type\": \"suite\", \
                     \"event\": \"ok\", \
                     \"passed\": 6, \
                     \"failed\": 5, \
                     \"allowed_fail\": 4, \
                     \"ignored\": 3, \
                     \"measured\": 2, \
                     \"filtered_out\": 1 }";
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn suite_failed() {
        let input = "{ \"type\": \"suite\", \
                     \"event\": \"failed\", \
                     \"passed\": 6, \
                     \"failed\": 5, \
                     \"allowed_fail\": 4, \
                     \"ignored\": 3, \
                     \"measured\": 2, \
                     \"filtered_out\": 1 }";
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn test_started() {
        let input = r#"{ "type": "test", "event": "started", "name": "foo" }"#;
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn test_timeout() {
        let input = r#"{ "type": "test", "event": "timeout", "name": "foo" }"#;
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn bench() {
        let input = "{ \"type\": \"bench\", \
                     \"name\": \"foo\", \
                     \"median\": 10, \
                     \"deviation\": 2 }";
        let _data: Event = serde_json::from_str(input).unwrap();
    }

    #[test]
    fn bench_full() {
        let input = "{ \"type\": \"bench\", \
                     \"name\": \"foo\", \
                     \"median\": 10, \
                     \"deviation\": 2, \
                     \"mib_per_second\": 1 }";
        let _data: Event = serde_json::from_str(input).unwrap();
    }
}
