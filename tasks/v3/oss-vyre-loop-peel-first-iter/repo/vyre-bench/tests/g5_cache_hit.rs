//! Cache hit rate test.
#![allow(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_cache_hit_rate() {
    let mut config = RunConfig::default();
    config.measured_samples = Some(30);
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];
    let registry = vyre_bench::registry::collect_all();

    // Run 1: Should have misses.
    let report1 = execute_suite(&registry, SuiteKind::Smoke, &config);
    let hit_rate1 = report1.summary.cache_hit_rate.unwrap_or(0.0);
    assert!(
        (0.0..=1.0).contains(&hit_rate1),
        "cache hit rate must be normalized: {hit_rate1}"
    );

    // Run 2: Should hit.
    let report2 = execute_suite(&registry, SuiteKind::Smoke, &config);
    let hit_rate2 = report2.summary.cache_hit_rate.unwrap_or(0.0);

    println!("{:#?}", report2.cases[0]);
    assert!(
        hit_rate2 > 0.95,
        "Second run should hit cache. Hit rate 1: {}, Hit rate 2: {}",
        hit_rate1,
        hit_rate2,
    );
}
