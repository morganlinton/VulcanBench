//! Integration test crate for the containing Vyre package.

use super::super::{PipelineCacheAudit, PipelineCacheAuditReport};

#[test]
fn empty_audit_reports_no_data_and_no_alarm() {
    let audit = PipelineCacheAudit::new();
    let report = audit.snapshot(8000);
    assert_eq!(
        report,
        PipelineCacheAuditReport {
            hits: 0,
            misses: 0,
            unknowns: 0,
            hit_rate_bps: None,
            below_alarm_threshold: false,
        }
    );
}

#[test]
fn audit_computes_hit_rate_bps_correctly() {
    let mut audit = PipelineCacheAudit::new();
    audit.observe(Some(true));
    audit.observe(Some(true));
    audit.observe(Some(true));
    audit.observe(Some(false));
    let report = audit.snapshot(0);
    assert_eq!(report.hits, 3);
    assert_eq!(report.misses, 1);
    assert_eq!(report.hit_rate_bps, Some(7500));
}

#[test]
fn audit_hit_rate_uses_widened_shared_ratio_for_saturated_counters() {
    let audit = PipelineCacheAudit {
        hits: u64::MAX,
        misses: 0,
        unknowns: 0,
    };
    let report = audit.snapshot(0);

    assert_eq!(report.hit_rate_bps, Some(10_000));
}

#[test]
fn audit_excludes_unknowns_from_rate_denominator() {
    let mut audit = PipelineCacheAudit::new();
    audit.observe(Some(true));
    audit.observe(None);
    audit.observe(None);
    audit.observe(Some(false));
    let report = audit.snapshot(0);
    assert_eq!(report.hits, 1);
    assert_eq!(report.misses, 1);
    assert_eq!(report.unknowns, 2);
    // 1/2 = 50%  -  unknowns must NOT dilute the rate.
    assert_eq!(report.hit_rate_bps, Some(5000));
}

#[test]
fn audit_alarms_when_hit_rate_below_threshold() {
    let mut audit = PipelineCacheAudit::new();
    for _ in 0..3 {
        audit.observe(Some(true));
    }
    for _ in 0..7 {
        audit.observe(Some(false));
    }
    let report = audit.snapshot(8000);
    assert_eq!(report.hit_rate_bps, Some(3000));
    assert!(report.below_alarm_threshold);
}

#[test]
fn audit_does_not_alarm_at_exactly_threshold() {
    let mut audit = PipelineCacheAudit::new();
    for _ in 0..8 {
        audit.observe(Some(true));
    }
    for _ in 0..2 {
        audit.observe(Some(false));
    }
    let report = audit.snapshot(8000);
    assert_eq!(report.hit_rate_bps, Some(8000));
    assert!(!report.below_alarm_threshold);
}

#[test]
fn audit_alarm_disabled_with_zero_threshold() {
    let mut audit = PipelineCacheAudit::new();
    for _ in 0..5 {
        audit.observe(Some(false));
    }
    let report = audit.snapshot(0);
    assert_eq!(report.hit_rate_bps, Some(0));
    assert!(!report.below_alarm_threshold);
}

#[test]
fn audit_no_alarm_when_no_data_even_with_threshold() {
    let mut audit = PipelineCacheAudit::new();
    audit.observe(None);
    audit.observe(None);
    let report = audit.snapshot(8000);
    assert_eq!(report.hit_rate_bps, None);
    assert!(!report.below_alarm_threshold);
}
