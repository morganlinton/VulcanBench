//! The Anthropic Messages provider, a port of `AnthropicProvider` in
//! `harness/agent/providers.py`: the same request shape (system split,
//! tool-result turns, prompt-cache breakpoints, output ceiling by effort),
//! the same retry policy, and the same token accounting (cache reads weighted
//! 0.1, cache writes 1.25, folded into the prompt count).

use std::time::Duration;

use crate::error::EngineError;
use crate::http::{
    budgeted_attempts, call_with_retry, http_timeout, post_json, HttpClient, ProviderFailure,
    DEFAULT_MAX_REQUEST_TIMEOUT_S,
};
use crate::json::{self, obj, Value};
use crate::provider::{LlmResponse, Provider, TokenUsage, ToolInvocation};
use crate::pyfmt::{py_repr, py_str};

/// Output ceiling by requested effort: thinking bills against `max_tokens`.
const MAX_TOKENS: [(&str, i64); 6] = [
    ("low", 32_000),
    ("medium", 64_000),
    ("high", 128_000),
    ("xhigh", 128_000),
    ("extra-high", 128_000),
    ("max", 128_000),
];
const MAX_TOKENS_DEFAULT: i64 = 32_000;
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    model: String,
    client: Box<dyn HttpClient>,
    sleep: fn(Duration),
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

fn cache_control() -> Value {
    obj! { "type" => "ephemeral" }
}

/// `_loads_args`: tool arguments as an object, parsing a JSON string if needed.
fn loads_args(raw: Option<&Value>) -> Value {
    match raw {
        Some(Value::Object(_)) => raw.cloned().unwrap_or_else(Value::object),
        Some(Value::Str(text)) => match json::parse(if text.is_empty() { "{}" } else { text }) {
            Ok(parsed @ Value::Object(_)) => parsed,
            _ => Value::object(),
        },
        _ => Value::object(),
    }
}

/// `_to_anthropic_messages`: split out the system prompt and convert
/// OpenAI-style turns to Anthropic blocks.
pub fn to_anthropic_messages(messages: &[Value]) -> (String, Vec<Value>) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for message in messages {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let content_text = message.get("content").map(py_str).unwrap_or_default();
        let tool_calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .filter(|calls| !calls.is_empty());
        if role == "system" {
            system_parts.push(content_text);
        } else if role == "tool" {
            out.push(obj! {
                "role" => "user",
                "content" => vec![obj! {
                    "type" => "tool_result",
                    "tool_use_id" => message.get("tool_call_id").map(py_str).unwrap_or_default(),
                    "content" => content_text,
                }],
            });
        } else if let (true, Some(calls)) = (role == "assistant", tool_calls) {
            let mut blocks = Vec::new();
            if message.get("content").is_some_and(truthy) {
                blocks.push(obj! { "type" => "text", "text" => content_text.clone() });
            }
            for call in calls {
                let function = call.get("function");
                blocks.push(obj! {
                    "type" => "tool_use",
                    "id" => call.get("id").map(py_str).unwrap_or_default(),
                    "name" => function.and_then(|f| f.get("name")).map(py_str).unwrap_or_default(),
                    "input" => loads_args(function.and_then(|f| f.get("arguments"))),
                });
            }
            out.push(obj! { "role" => "assistant", "content" => blocks });
        } else {
            let role = if role.is_empty() { "user" } else { role };
            out.push(obj! { "role" => role, "content" => content_text });
        }
    }
    (system_parts.join("\n"), out)
}

/// Python truthiness for the values a chat message can hold.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Int(0) => false,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(entries) => !entries.is_empty(),
        _ => true,
    }
}

/// `_with_prompt_caching`: an ephemeral breakpoint on the system prefix and
/// on the tail of the last message.
pub fn with_prompt_caching(system: &str, mut converted: Vec<Value>) -> (Value, Vec<Value>) {
    let system_field = if system.is_empty() {
        Value::Str(String::new())
    } else {
        Value::Array(vec![obj! {
            "type" => "text",
            "text" => system,
            "cache_control" => cache_control(),
        }])
    };
    if let Some(last) = converted.last_mut() {
        let content = last.get("content").cloned().unwrap_or(Value::Null);
        match content {
            Value::Str(text) => {
                last.insert(
                    "content",
                    vec![obj! { "type" => "text", "text" => text, "cache_control" => cache_control() }],
                );
            }
            Value::Array(mut blocks) if !blocks.is_empty() => {
                if let Some(tail) = blocks.last_mut() {
                    tail.insert("cache_control", cache_control());
                }
                last.insert("content", blocks);
            }
            _ => {}
        }
    }
    (system_field, converted)
}

/// `_openai_tool_to_anthropic`.
pub fn openai_tool_to_anthropic(tool: &Value) -> Value {
    let function = tool.get("function");
    obj! {
        "name" => function.and_then(|f| f.get("name")).cloned().unwrap_or(Value::Null),
        "description" => function.and_then(|f| f.get("description")).cloned().unwrap_or_else(|| Value::Str(String::new())),
        "input_schema" => function
            .and_then(|f| f.get("parameters"))
            .cloned()
            .unwrap_or_else(|| obj! { "type" => "object", "properties" => Value::object() }),
    }
}

fn max_tokens_for(effort: Option<&str>) -> i64 {
    effort
        .and_then(|e| MAX_TOKENS.iter().find(|(name, _)| *name == e))
        .map_or(MAX_TOKENS_DEFAULT, |(_, tokens)| *tokens)
}

impl AnthropicProvider {
    pub fn new(model: &str, client: Box<dyn HttpClient>) -> Self {
        Self {
            model: model.to_owned(),
            client,
            sleep: std::thread::sleep,
        }
    }

    /// The Messages API payload for one step (pure, so tests can check the shape).
    pub fn build_payload(
        &self,
        messages: &[Value],
        tools: &Value,
        effort: Option<&str>,
        fallback: Option<&str>,
    ) -> Value {
        let (system, converted) = to_anthropic_messages(messages);
        let (system_field, converted) = with_prompt_caching(&system, converted);
        let mut payload = obj! {
            "model" => self.model.as_str(),
            "max_tokens" => max_tokens_for(effort),
            "messages" => converted,
        };
        if let Some(effort) = effort {
            payload.insert("output_config", obj! { "effort" => effort });
        }
        if truthy(&system_field) {
            payload.insert("system", system_field);
        }
        if let Some(tool_list) = tools.as_array().filter(|t| !t.is_empty()) {
            let converted: Vec<Value> = tool_list.iter().map(openai_tool_to_anthropic).collect();
            payload.insert("tools", converted);
        }
        if let Some(model) = fallback {
            payload.insert("fallbacks", vec![obj! { "model" => model }]);
        }
        payload
    }

    fn complete_once(
        &self,
        messages: &[Value],
        tools: &Value,
        timeout: f64,
        effort: Option<&str>,
    ) -> Result<LlmResponse, ProviderFailure> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ProviderFailure::retryable("ANTHROPIC_API_KEY is not set"))?;
        let base = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_owned());
        let base = base.trim_end_matches('/');
        let fallback = std::env::var("VULCANBENCH_REFUSAL_FALLBACK")
            .ok()
            .filter(|f| !f.is_empty());
        let payload = self.build_payload(messages, tools, effort, fallback.as_deref());
        let max_tokens = payload
            .get("max_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(MAX_TOKENS_DEFAULT);
        let mut headers = vec![
            ("x-api-key".to_owned(), api_key),
            ("anthropic-version".to_owned(), API_VERSION.to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        if fallback.is_some() {
            headers.push((
                "anthropic-beta".to_owned(),
                "server-side-fallback-2026-06-01".to_owned(),
            ));
        }
        let url = format!("{base}/v1/messages");
        let body = post_json(self.client.as_ref(), &url, &headers, &payload, timeout)?;

        let served = body.get("model").map(py_str).filter(|s| !s.is_empty());
        if let (Some(_), Some(served)) = (&fallback, &served) {
            if !served.starts_with(&self.model) {
                println!("[vulcanbench] refusal fallback: step served by {served}");
            }
        }
        let stop_reason = body.get("stop_reason").and_then(Value::as_str);
        if stop_reason == Some("refusal") {
            let details = body
                .get("stop_details")
                .cloned()
                .unwrap_or_else(Value::object);
            let category = details.get("category").cloned().unwrap_or(Value::Null);
            let explanation = details
                .get("explanation")
                .map(py_str)
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| "no explanation provided".to_owned());
            return Err(ProviderFailure::retryable(format!(
                "model refused the request (category={}): {explanation}",
                py_repr(&category)
            )));
        }
        let mut content_text: Vec<String> = Vec::new();
        let mut tool_calls = Vec::new();
        for block in body.get("content").and_then(Value::as_array).unwrap_or(&[]) {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    content_text.push(block.get("text").map(py_str).unwrap_or_default())
                }
                Some("tool_use") => tool_calls.push(ToolInvocation {
                    id: block.get("id").map(py_str).unwrap_or_default(),
                    name: block.get("name").map(py_str).unwrap_or_default(),
                    arguments: block
                        .get("input")
                        .filter(|input| truthy(input))
                        .cloned()
                        .unwrap_or_else(Value::object),
                }),
                _ => {}
            }
        }
        if stop_reason == Some("max_tokens") && tool_calls.is_empty() {
            return Err(ProviderFailure::fatal(format!(
                "response truncated at max_tokens={max_tokens} with no tool call (stop_reason=max_tokens)"
            )));
        }
        let usage = body.get("usage").cloned().unwrap_or_else(Value::object);
        let number = |key: &str| usage.get(key).and_then(Value::as_f64).unwrap_or(0.0);
        let effective_prompt = (number("input_tokens")
            + number("cache_read_input_tokens") * 0.1
            + number("cache_creation_input_tokens") * 1.25)
            .round_ties_even();
        let joined = content_text.join("\n");
        Ok(LlmResponse {
            content: (!joined.is_empty()).then_some(joined),
            tool_calls,
            usage: TokenUsage {
                prompt_tokens: effective_prompt as i64,
                completion_tokens: usage
                    .get("output_tokens")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            },
        })
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn complete(
        &self,
        messages: &[Value],
        tools: &Value,
        timeout_s: Option<f64>,
        effort: Option<&str>,
    ) -> Result<LlmResponse, EngineError> {
        let timeout = http_timeout(timeout_s, DEFAULT_MAX_REQUEST_TIMEOUT_S)?;
        let attempts = budgeted_attempts(timeout_s, timeout);
        let sleep = self.sleep;
        call_with_retry(attempts, &|d| sleep(d), || {
            self.complete_once(messages, tools, timeout, effort)
        })
        .map_err(EngineError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpError, HttpRequest, HttpResponse};
    use crate::protocol::openai_tool_schemas;
    use std::cell::RefCell;

    struct Capture {
        requests: RefCell<Vec<HttpRequest>>,
        responses: RefCell<Vec<Result<HttpResponse, HttpError>>>,
    }

    impl HttpClient for Capture {
        fn post(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
            self.requests.borrow_mut().push(request.clone());
            self.responses.borrow_mut().remove(0)
        }
    }

    fn transcript() -> Vec<Value> {
        vec![
            obj! { "role" => "system", "content" => "be terse" },
            obj! { "role" => "user", "content" => "# Issue\n\nfix it" },
            obj! {
                "role" => "assistant",
                "content" => "Reading.",
                "tool_calls" => vec![obj! {
                    "id" => "c1",
                    "type" => "function",
                    "function" => obj! { "name" => "read_file", "arguments" => "{\"path\": \"a.py\"}" },
                }],
            },
            obj! { "role" => "tool", "tool_call_id" => "c1", "content" => "{\"result\": \"x\"}" },
        ]
    }

    #[test]
    fn payload_shape_matches_providers_py() {
        let provider = AnthropicProvider::new(
            "claude-opus-5-5",
            Box::new(Capture {
                requests: RefCell::new(vec![]),
                responses: RefCell::new(vec![]),
            }),
        );
        let payload =
            provider.build_payload(&transcript(), &openai_tool_schemas(), Some("xhigh"), None);
        let keys: Vec<&str> = payload
            .as_object()
            .expect("obj")
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            keys,
            [
                "model",
                "max_tokens",
                "messages",
                "output_config",
                "system",
                "tools"
            ]
        );
        assert_eq!(payload.get("max_tokens"), Some(&Value::Int(128_000)));
        assert_eq!(
            payload.get("system").map(Value::to_json),
            Some(
                r#"[{"type": "text", "text": "be terse", "cache_control": {"type": "ephemeral"}}]"#
                    .to_owned()
            )
        );
        let messages = payload
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[1].to_json(),
            r#"{"role": "assistant", "content": [{"type": "text", "text": "Reading."}, {"type": "tool_use", "id": "c1", "name": "read_file", "input": {"path": "a.py"}}]}"#
        );
        assert_eq!(
            messages[2].to_json(),
            r#"{"role": "user", "content": [{"type": "tool_result", "tool_use_id": "c1", "content": "{\"result\": \"x\"}", "cache_control": {"type": "ephemeral"}}]}"#
        );
        let tools = payload
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools");
        assert_eq!(
            tools[0].get("name"),
            Some(&Value::Str("list_files".to_owned()))
        );
        assert!(tools[0].get("input_schema").is_some());
        let plain = provider.build_payload(
            &[obj! { "role" => "user", "content" => "hi" }],
            &Value::Array(vec![]),
            None,
            Some("claude-opus-4-8"),
        );
        assert_eq!(plain.get("max_tokens"), Some(&Value::Int(32_000)));
        assert!(plain.get("output_config").is_none());
        assert!(plain.get("system").is_none());
        assert!(plain.get("tools").is_none());
        assert_eq!(
            plain.get("fallbacks").map(Value::to_json),
            Some(r#"[{"model": "claude-opus-4-8"}]"#.to_owned())
        );
    }

    #[test]
    fn response_parsing_and_token_weighting() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-for-unit-test");
        std::env::remove_var("VULCANBENCH_REFUSAL_FALLBACK");
        let body = r#"{"model": "claude-opus-5-5", "stop_reason": "tool_use", "content": [{"type": "text", "text": "Thinking."}, {"type": "tool_use", "id": "toolu_1", "name": "edit_file", "input": {"path": "a.py", "old_string": "", "new_string": "x"}}], "usage": {"input_tokens": 100, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 200, "output_tokens": 55}}"#;
        let capture = Capture {
            requests: RefCell::new(vec![]),
            responses: RefCell::new(vec![Ok(HttpResponse {
                status: 200,
                body: body.to_owned(),
            })]),
        };
        let provider = AnthropicProvider::new("claude-opus-5-5", Box::new(capture));
        let response = provider
            .complete(
                &transcript(),
                &openai_tool_schemas(),
                Some(300.0),
                Some("high"),
            )
            .expect("parsed");
        assert_eq!(response.content.as_deref(), Some("Thinking."));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "edit_file");
        // 100 + 1000 * 0.1 + 200 * 1.25 = 450
        assert_eq!(
            response.usage,
            TokenUsage {
                prompt_tokens: 450,
                completion_tokens: 55
            }
        );

        let truncated = r#"{"stop_reason": "max_tokens", "content": [{"type": "text", "text": "..."}], "usage": {}}"#;
        let provider = AnthropicProvider::new(
            "claude-opus-5-5",
            Box::new(Capture {
                requests: RefCell::new(vec![]),
                responses: RefCell::new(vec![Ok(HttpResponse {
                    status: 200,
                    body: truncated.to_owned(),
                })]),
            }),
        );
        let err = provider
            .complete(&transcript(), &Value::Array(vec![]), None, Some("low"))
            .expect_err("truncation is final");
        assert_eq!(
            err.to_string(),
            "response truncated at max_tokens=32000 with no tool call (stop_reason=max_tokens)"
        );
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn headers_carry_the_key_and_refusals_are_retryable() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-for-unit-test");
        std::env::set_var("ANTHROPIC_BASE_URL", "https://proxy.example.test/");
        let refusal = r#"{"stop_reason": "refusal", "stop_details": {"category": "harm", "explanation": "no"}, "content": [], "usage": {}}"#;
        let ok = r#"{"stop_reason": "end_turn", "content": [{"type": "text", "text": "FINISH: done"}], "usage": {"input_tokens": 10, "output_tokens": 5}}"#;
        let mut provider = AnthropicProvider::new(
            "claude-sonnet-5-5",
            Box::new(Capture {
                requests: RefCell::new(vec![]),
                responses: RefCell::new(vec![
                    Ok(HttpResponse {
                        status: 200,
                        body: refusal.to_owned(),
                    }),
                    Ok(HttpResponse {
                        status: 200,
                        body: ok.to_owned(),
                    }),
                ]),
            }),
        );
        provider.sleep = |_| {};
        let response = provider
            .complete(&transcript(), &Value::Array(vec![]), None, None)
            .expect("second attempt succeeds");
        assert_eq!(response.content.as_deref(), Some("FINISH: done"));
        assert_eq!(response.usage.prompt_tokens, 10);
        std::env::remove_var("ANTHROPIC_BASE_URL");
    }
}
