#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick_cache::json_escape;
use std::time::Duration;

pub(crate) fn mutation_cache_json(
    source_hash: &str,
    test_hash: &str,
    mutation: &str,
    outcome: &str,
    wall_time: Duration,
) -> String {
    format!(
        "{{\"source_hash\":\"{}\",\"test_hash\":\"{}\",\"mutation\":\"{}\",\"outcome\":\"{}\",\"wall_time_ms\":{}}}\n",
        json_escape(source_hash),
        json_escape(test_hash),
        json_escape(mutation),
        json_escape(outcome),
        wall_time.as_millis()
    )
}
