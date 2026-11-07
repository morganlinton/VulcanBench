//! Reasoning-effort metadata, the slice of `harness/effort.py` the mock
//! provider needs: the vocabulary check and the recorded-but-not-sent record.

use crate::error::EngineError;
use crate::json::{obj, Value};
use crate::pyfmt::py_repr_str;

pub const EFFORT_LEVELS: [&str; 7] = [
    "minimal",
    "low",
    "medium",
    "high",
    "extra-high",
    "max",
    "ultra",
];

/// What a run recorded about its effort request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortConfig {
    pub requested: String,
    pub provider: String,
    pub provider_value: Option<String>,
    pub supported: bool,
}

impl EffortConfig {
    pub fn as_summary(&self) -> Value {
        obj! {
            "requested" => self.requested.as_str(),
            "provider" => self.provider.as_str(),
            "provider_value" => self.provider_value.as_deref(),
            "supported" => self.supported,
        }
    }
}

/// Lowercases and validates a user-facing effort string.
///
/// # Errors
///
/// `EngineError::Invalid` when the level is outside the vocabulary.
pub fn normalize_effort(value: Option<&str>) -> Result<Option<String>, EngineError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim().to_lowercase();
    if !EFFORT_LEVELS.contains(&normalized.as_str()) {
        let mut allowed: Vec<&str> = EFFORT_LEVELS.to_vec();
        allowed.sort_unstable();
        return Err(EngineError::Invalid(format!(
            "effort must be one of {}, got {}",
            allowed.join(", "),
            py_repr_str(value)
        )));
    }
    Ok(Some(normalized))
}

/// Resolves effort metadata for a provider. Only the providers the spike can
/// run are mapped; the mock never sends effort, so every level is recorded
/// but not sent.
///
/// # Errors
///
/// Invalid vocabulary, or a provider whose effort mapping was not ported.
pub fn effort_config(
    provider: &str,
    requested: Option<&str>,
) -> Result<Option<EffortConfig>, EngineError> {
    let Some(effort) = normalize_effort(requested)? else {
        return Ok(None);
    };
    let provider_name = provider.trim().to_lowercase();
    match provider_name.as_str() {
        "mock" | "ollama" | "openrouter" => Ok(Some(EffortConfig {
            requested: effort,
            provider: provider_name,
            provider_value: None,
            supported: false,
        })),
        // Anthropic Messages API `output_config.effort`; `max` and `minimal`
        // are recorded but not sent, exactly as in harness/effort.py.
        "anthropic" => {
            let provider_value = match effort.as_str() {
                "low" | "medium" | "high" => Some(effort.clone()),
                "extra-high" => Some("xhigh".to_owned()),
                _ => None,
            };
            Ok(Some(EffortConfig {
                requested: effort,
                provider: provider_name,
                supported: provider_value.is_some(),
                provider_value,
            }))
        }
        _ => Err(EngineError::Invalid(format!(
            "effort mapping for provider {} is not ported in the Rust spike",
            py_repr_str(provider)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_records_but_never_sends_effort() {
        let cfg = effort_config("mock", Some(" High "))
            .expect("valid")
            .expect("present");
        assert_eq!(
            cfg.as_summary().to_json(),
            r#"{"requested": "high", "provider": "mock", "provider_value": null, "supported": false}"#
        );
        assert!(effort_config("mock", None).expect("valid").is_none());
        let err = normalize_effort(Some("turbo")).expect_err("not a level");
        assert_eq!(
            err.to_string(),
            "effort must be one of extra-high, high, low, max, medium, minimal, ultra, got 'turbo'"
        );
    }
}
