//! The provider contract, the deterministic `mock` provider, and the spec
//! parser (`harness/agent/providers.py`). The Anthropic provider lives in
//! [`crate::anthropic`]; every other provider is still unported.

use crate::anthropic::AnthropicProvider;
use crate::curl::CurlClient;
use crate::error::EngineError;
use crate::json::{obj, Value};
use crate::pyfmt::{py_repr_str, py_str};

/// A single tool call requested by the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

impl ToolInvocation {
    pub fn to_dict(&self) -> Value {
        obj! {
            "id" => self.id.as_str(),
            "name" => self.name.as_str(),
            "arguments" => self.arguments.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

impl TokenUsage {
    pub fn to_dict(self) -> Value {
        obj! {
            "prompt_tokens" => self.prompt_tokens,
            "completion_tokens" => self.completion_tokens,
        }
    }
}

/// Uniform response shape across providers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolInvocation>,
    pub usage: TokenUsage,
}

impl LlmResponse {
    pub fn wants_tools(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// A model backend. Messages are the OpenAI-style chat dicts the loop builds;
/// `timeout_s` is the run's remaining budget and `effort` the provider's own
/// value for the requested level, when it supports one.
pub trait Provider {
    fn name(&self) -> &str;

    /// # Errors
    ///
    /// `EngineError::Provider` when no response can be produced.
    fn complete(
        &self,
        messages: &[Value],
        tools: &Value,
        timeout_s: Option<f64>,
        effort: Option<&str>,
    ) -> Result<LlmResponse, EngineError>;
}

/// Deterministic, offline provider: read the issue, create `hello.py`, run
/// the tests, then finish. Lets the real loop run end to end with no network.
#[derive(Debug, Default)]
pub struct MockProvider;

fn content_text(message: &Value) -> String {
    message.get("content").map(py_str).unwrap_or_default()
}

impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn complete(
        &self,
        messages: &[Value],
        _tools: &Value,
        _timeout_s: Option<f64>,
        _effort: Option<&str>,
    ) -> Result<LlmResponse, EngineError> {
        if messages
            .iter()
            .any(|m| content_text(m).contains("VULCANBENCH_JUDGE"))
        {
            return Ok(LlmResponse {
                content: Some(r#"{"score": 80, "rationale": "mock judge"}"#.to_owned()),
                tool_calls: Vec::new(),
                usage: TokenUsage {
                    prompt_tokens: 120,
                    completion_tokens: 15,
                },
            });
        }
        if messages
            .iter()
            .any(|m| content_text(m).contains("VULCANBENCH_GRADER"))
        {
            let has_change = messages
                .iter()
                .any(|m| content_text(m).contains("diff --git"));
            let verdict = if has_change { "true" } else { "false" };
            let arr = format!("[{}]", vec![verdict; 16].join(", "));
            return Ok(LlmResponse {
                content: Some(format!(
                    "{{\"correct\": {verdict}, \"confidence\": 0.9, \"reasons\": \"mock grader\", \
                     \"blocking\": {arr}, \"weighted\": {arr}, \"notes\": \"mock grader\"}}"
                )),
                tool_calls: Vec::new(),
                usage: TokenUsage {
                    prompt_tokens: 140,
                    completion_tokens: 18,
                },
            });
        }
        let steps = messages
            .iter()
            .filter(|m| m.get("role").and_then(Value::as_str) == Some("tool"))
            .count();
        let usage = TokenUsage {
            prompt_tokens: 50 + i64::try_from(steps).unwrap_or(0) * 20,
            completion_tokens: 20,
        };
        let response = match steps {
            0 => LlmResponse {
                content: Some("Reading the issue.".to_owned()),
                tool_calls: vec![ToolInvocation {
                    id: "c1".to_owned(),
                    name: "read_file".to_owned(),
                    arguments: obj! { "path" => "issue.md" },
                }],
                usage,
            },
            1 => LlmResponse {
                content: Some("Creating hello.py with the required output.".to_owned()),
                tool_calls: vec![ToolInvocation {
                    id: "c2".to_owned(),
                    name: "edit_file".to_owned(),
                    arguments: obj! {
                        "path" => "hello.py",
                        "old_string" => "",
                        "new_string" => "print(\"hello from vulcanbench\")\n",
                    },
                }],
                usage,
            },
            2 => LlmResponse {
                content: Some("Running tests.".to_owned()),
                tool_calls: vec![ToolInvocation {
                    id: "c3".to_owned(),
                    name: "run_tests".to_owned(),
                    arguments: Value::object(),
                }],
                usage,
            },
            _ => LlmResponse {
                content: Some("FINISH: implemented and verified.".to_owned()),
                tool_calls: Vec::new(),
                usage,
            },
        };
        Ok(response)
    }
}

/// Splits `provider:model` into its parts.
///
/// # Errors
///
/// `EngineError::Invalid` with the Python message when the spec is malformed.
pub fn parse_model_spec(spec: &str) -> Result<(String, String), EngineError> {
    let malformed = || {
        EngineError::Invalid(format!(
            "model spec must be 'provider:model', got {}",
            py_repr_str(spec)
        ))
    };
    let (provider, model) = spec.split_once(':').ok_or_else(malformed)?;
    let provider = provider.trim().to_lowercase();
    let model = model.trim().to_owned();
    if provider.is_empty() || model.is_empty() {
        return Err(malformed());
    }
    Ok((provider, model))
}

/// Constructs the provider for a spec: `mock`, or `anthropic` over curl.
///
/// # Errors
///
/// A malformed spec, or a provider the spike does not implement.
pub fn get_provider(spec: &str) -> Result<Box<dyn Provider>, EngineError> {
    let (provider, model) = parse_model_spec(spec)?;
    match provider.as_str() {
        "mock" => Ok(Box::new(MockProvider)),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(
            &model,
            Box::new(CurlClient::from_environment()),
        ))),
        other => Err(EngineError::Invalid(format!(
            "provider {} is not implemented in the Rust spike; known: anthropic, mock",
            py_repr_str(other)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_policy_is_driven_by_tool_message_count() {
        let provider = MockProvider;
        let tools = Value::Array(Vec::new());
        let mut messages = vec![obj! { "role" => "system", "content" => "x" }];
        let first = provider
            .complete(&messages, &tools, None, None)
            .expect("ok");
        assert_eq!(first.tool_calls[0].name, "read_file");
        assert_eq!(first.usage.prompt_tokens, 50);
        for _ in 0..3 {
            messages.push(obj! { "role" => "tool", "content" => "{}" });
        }
        let last = provider
            .complete(&messages, &tools, Some(1.0), Some("high"))
            .expect("ok");
        assert!(!last.wants_tools());
        assert_eq!(
            last.content.as_deref(),
            Some("FINISH: implemented and verified.")
        );
        assert_eq!(last.usage.prompt_tokens, 110);
    }

    #[test]
    fn spec_parsing() {
        assert_eq!(
            parse_model_spec(" Mock:synthetic ").expect("ok"),
            ("mock".to_owned(), "synthetic".to_owned())
        );
        assert_eq!(
            parse_model_spec("nocolon").expect_err("bad").to_string(),
            "model spec must be 'provider:model', got 'nocolon'"
        );
        assert!(get_provider("openai:gpt").is_err());
        assert_eq!(
            get_provider("anthropic:claude-opus-5-5")
                .expect("built")
                .name(),
            "anthropic"
        );
    }
}
