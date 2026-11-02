//! Per-model token pricing (`harness/pricing.py`), ported for the providers
//! the spike can run: the Anthropic rows, the free `mock:` and `ollama:`
//! prefixes, the spec aliases, and the `VULCANBENCH_PRICING` override file.
//! Every other spec is unpriced, which the summary records honestly as
//! `cost_usd: null` and `model_priced: false`.

use std::sync::OnceLock;

use crate::json::{self, Value};
use crate::pyfmt::py_round;

/// USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rate {
    pub input: f64,
    pub cached_input: Option<f64>,
    pub output: f64,
}

const fn rate(input: f64, output: f64) -> Rate {
    Rate {
        input,
        cached_input: None,
        output,
    }
}

const fn cached(input: f64, cached_input: f64, output: f64) -> Rate {
    Rate {
        input,
        cached_input: Some(cached_input),
        output,
    }
}

/// The subset of `PRICES` the spike carries, same numbers as the Python table.
const PRICES: [(&str, Rate); 14] = [
    ("anthropic:claude-fable-5", rate(10.00, 50.00)),
    ("anthropic:claude-fable-5-1", rate(10.00, 50.00)),
    ("anthropic:claude-opus-5-5", cached(4.00, 0.20, 20.00)),
    ("anthropic:claude-opus-5", rate(5.00, 25.00)),
    ("anthropic:claude-opus-4-8", rate(5.00, 25.00)),
    ("anthropic:claude-opus-4-7", rate(5.00, 25.00)),
    ("anthropic:claude-opus-4-6", rate(5.00, 25.00)),
    ("anthropic:claude-sonnet-5", rate(3.00, 15.00)),
    ("anthropic:claude-sonnet-5-5", cached(2.00, 0.20, 10.00)),
    ("anthropic:claude-sonnet-4-6", rate(3.00, 15.00)),
    ("anthropic:claude-haiku-4-5", rate(1.00, 5.00)),
    ("anthropic:claude-haiku-5-5", cached(0.10, 0.01, 0.50)),
    ("mock:", rate(0.0, 0.0)),
    ("ollama:", rate(0.0, 0.0)),
];

const SPEC_ALIASES: [(&str, &str); 7] = [
    ("claude-code:", "anthropic:"),
    ("codex:", "openai:"),
    ("grok-build:", "xai:"),
    ("zcode:", "zai:"),
    ("muse-code:", "meta:"),
    ("pi:meta/", "meta:"),
    ("dsh:", "deepseek:"),
];

const PER_MILLION: f64 = 1_000_000.0;

fn canonical_spec(model: &str) -> String {
    for (prefix, replacement) in SPEC_ALIASES {
        if let Some(rest) = model.strip_prefix(prefix) {
            return format!("{replacement}{rest}");
        }
    }
    model.to_owned()
}

fn rate_from_value(value: &Value) -> Option<Rate> {
    let input = value.get("input").and_then(Value::as_f64)?;
    let output = value.get("output").and_then(Value::as_f64)?;
    Some(Rate {
        input,
        cached_input: value.get("cached_input").and_then(Value::as_f64),
        output,
    })
}

/// The built-in table merged with `VULCANBENCH_PRICING` (a JSON file whose
/// entries override or extend it). Read once per process, like Python's cache.
fn prices() -> &'static Vec<(String, Rate)> {
    static CACHE: OnceLock<Vec<(String, Rate)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut table: Vec<(String, Rate)> = PRICES
            .iter()
            .map(|(spec, r)| ((*spec).to_owned(), *r))
            .collect();
        let Some(path) = std::env::var_os("VULCANBENCH_PRICING") else {
            return table;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return table;
        };
        let Ok(Value::Object(entries)) = json::parse(&text) else {
            return table;
        };
        for (spec, value) in entries {
            let position = table.iter().position(|(s, _)| *s == spec);
            let merged = match (position, &value) {
                (Some(i), Value::Object(_)) => {
                    let existing = table[i].1;
                    Some(Rate {
                        input: value
                            .get("input")
                            .and_then(Value::as_f64)
                            .unwrap_or(existing.input),
                        cached_input: value
                            .get("cached_input")
                            .and_then(Value::as_f64)
                            .or(existing.cached_input),
                        output: value
                            .get("output")
                            .and_then(Value::as_f64)
                            .unwrap_or(existing.output),
                    })
                }
                (_, Value::Object(_)) => rate_from_value(&value),
                _ => None,
            };
            match (position, merged) {
                (Some(i), Some(r)) => table[i].1 = r,
                (Some(i), None) => {
                    table.remove(i);
                }
                (None, Some(r)) => table.push((spec, r)),
                (None, None) => {}
            }
        }
        table
    })
}

/// The effective rate for a spec: exact match first, then the `provider:` prefix.
pub fn rate_for(model: &str) -> Option<Rate> {
    let spec = canonical_spec(model);
    let table = prices();
    if let Some((_, r)) = table.iter().find(|(s, _)| *s == spec) {
        return Some(*r);
    }
    let provider = spec.split_once(':').map(|(p, _)| format!("{p}:"))?;
    table.iter().find(|(s, _)| *s == provider).map(|(_, r)| *r)
}

/// USD cost of a call, or `None` when the model is unpriced. Cached input
/// bills at `cached_input` when the table provides it, else at the input rate.
pub fn cost_usd(
    model: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_input_tokens: i64,
) -> Option<f64> {
    let r = rate_for(model)?;
    let prompt = prompt_tokens.max(0);
    let cached_tokens = cached_input_tokens.clamp(0, prompt);
    let uncached = prompt - cached_tokens;
    let cached_rate = r.cached_input.unwrap_or(r.input);
    let cost = (uncached as f64 * r.input
        + cached_tokens as f64 * cached_rate
        + completion_tokens.max(0) as f64 * r.output)
        / PER_MILLION;
    Some(py_round(cost, 6))
}

pub fn is_priced(model: &str) -> bool {
    rate_for(model).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_and_costs_match_the_python_table() {
        assert_eq!(cost_usd("mock:synthetic", 320, 80, 0), Some(0.0));
        assert!(is_priced("mock:synthetic"));
        assert!(!is_priced("openai:gpt-6-sol"));
        // Opus 5.5: 1,000,000 input at $4 plus 100,000 output at $20 = $6.
        assert_eq!(
            cost_usd("anthropic:claude-opus-5-5", 1_000_000, 100_000, 0),
            Some(6.0)
        );
        // Half the input cached at $0.20: 500k * 4 + 500k * 0.2 = 2.1, plus 2.0 output.
        assert_eq!(
            cost_usd("anthropic:claude-opus-5-5", 1_000_000, 100_000, 500_000),
            Some(4.1)
        );
        assert_eq!(
            cost_usd("claude-code:claude-opus-5", 1_000_000, 0, 0),
            Some(5.0)
        );
        assert_eq!(
            cost_usd("anthropic:claude-unknown-9", 1_000_000, 0, 0),
            None
        );
        assert_eq!(
            canonical_spec("pi:meta/muse-spark-1.3"),
            "meta:muse-spark-1.3"
        );
    }
}
