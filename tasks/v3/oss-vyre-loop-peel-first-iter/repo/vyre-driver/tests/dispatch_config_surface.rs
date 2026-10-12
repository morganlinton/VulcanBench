//! Surface tests for `DispatchConfig`.
//!
//! Dispatch configuration drives how kernels are launched; incorrect
//! defaults or missing fields can cause dispatch failures.

use std::time::Duration;
use vyre::backend::DispatchConfig;

#[test]
fn dispatch_config_default_is_constructible() {
    let _ = DispatchConfig::default();
}

#[test]
fn dispatch_config_new_sets_profile() {
    let config = DispatchConfig::new(Some("stress".to_string()), None, None, None);
    assert_eq!(config.profile, Some("stress".to_string()));
}

#[test]
fn dispatch_config_new_sets_ulp_budget() {
    let config = DispatchConfig::new(None, Some(4), None, None);
    assert_eq!(config.ulp_budget, Some(4));
}

#[test]
fn dispatch_config_new_sets_timeout() {
    let config = DispatchConfig::new(None, None, Some(Duration::from_secs(30)), None);
    assert_eq!(config.timeout, Some(Duration::from_secs(30)));
}

#[test]
fn dispatch_config_new_sets_label() {
    let config = DispatchConfig::new(None, None, None, Some("my-kernel".to_string()));
    assert_eq!(config.label, Some("my-kernel".to_string()));
}

#[test]
fn dispatch_config_clone_matches_original() {
    let config = DispatchConfig::new(
        Some("default".to_string()),
        Some(2),
        None,
        Some("test".to_string()),
    );
    let cloned = config.clone();
    assert_eq!(config.profile, cloned.profile);
    assert_eq!(config.ulp_budget, cloned.ulp_budget);
    assert_eq!(config.label, cloned.label);
}

#[test]
fn dispatch_config_default_has_no_overrides() {
    let config = DispatchConfig::default();
    assert!(config.workgroup_override.is_none());
    assert!(config.grid_override.is_none());
    assert!(config.profile.is_none());
    assert!(config.ulp_budget.is_none());
    assert!(config.timeout.is_none());
    assert!(config.label.is_none());
    assert!(config.max_output_bytes.is_none());
    assert!(config.fixpoint_iterations.is_none());
    assert!(config.speculation.is_none());
}
