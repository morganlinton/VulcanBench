//! Tool protocol: the tool schemas the model sees and the lax argument
//! coercion of `harness/agent/protocol.py` (`Model.from_args` and `coerce`).
//!
//! Arguments arrive from a model's JSON, so unknown keys are dropped, missing
//! required keys and wrong types are reported as errors (which become an
//! error observation for the model), and scalars coerce leniently: `"120"`
//! becomes `120` and `"true"` becomes `true`.

use crate::json::{obj, Value};
use crate::pyfmt::{py_repr, py_type_name};

/// Tools the loop dispatches; everything else is "unknown tool".
pub const VALID_TOOLS: [&str; 6] = [
    "list_files",
    "read_file",
    "search_code",
    "edit_file",
    "run_command",
    "run_tests",
];

const TRUE_WORDS: [&str; 6] = ["true", "1", "yes", "y", "on", "t"];
const FALSE_WORDS: [&str; 6] = ["false", "0", "no", "n", "off", "f"];

/// The OpenAI-style function schemas handed to the provider.
pub fn openai_tool_schemas() -> Value {
    fn tool(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
        let params = obj! {
            "type" => "object",
            "properties" => properties,
            "required" => required.into_iter().map(Value::from).collect::<Vec<_>>(),
        };
        obj! {
            "type" => "function",
            "function" => obj! {
                "name" => name,
                "description" => description,
                "parameters" => params,
            },
        }
    }
    Value::Array(vec![
        tool(
            "list_files",
            "List files in a directory",
            obj! {
                "dir" => obj! { "type" => "string", "default" => "." },
                "recursive" => obj! { "type" => "boolean", "default" => false },
            },
            vec![],
        ),
        tool(
            "read_file",
            "Read file contents, optionally with line range",
            obj! {
                "path" => obj! { "type" => "string" },
                "start_line" => obj! { "type" => "integer" },
                "limit" => obj! { "type" => "integer" },
            },
            vec!["path"],
        ),
        tool(
            "search_code",
            "Search code with ripgrep or semantic (future)",
            obj! {
                "query" => obj! { "type" => "string" },
                "glob" => obj! { "type" => "string" },
                "semantic" => obj! { "type" => "boolean", "default" => false },
            },
            vec!["query"],
        ),
        tool(
            "edit_file",
            "Edit file with exact string match, returns unified diff",
            obj! {
                "path" => obj! { "type" => "string" },
                "old_string" => obj! { "type" => "string" },
                "new_string" => obj! { "type" => "string" },
            },
            vec!["path", "old_string", "new_string"],
        ),
        tool(
            "run_command",
            "Run shell command, capture stdout/stderr/exit",
            obj! {
                "cmd" => obj! { "type" => "string" },
                "cwd" => obj! { "type" => "string" },
                "timeout" => obj! { "type" => "integer", "default" => 120 },
            },
            vec!["cmd"],
        ),
        obj! {
            "type" => "function",
            "function" => obj! {
                "name" => "run_tests",
                "description" => "Run task-specific tests via verifier or script",
                "parameters" => obj! { "type" => "object", "properties" => Value::object() },
            },
        },
    ])
}

/// A tool request as the executor sees it.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool: String,
    pub args: Value,
}

/// What a tool produced: a JSON-able result or an error message.
#[derive(Debug, Clone)]
pub struct ToolObservation {
    pub result: Value,
    pub error: Option<String>,
}

impl ToolObservation {
    pub fn ok(result: Value) -> Self {
        Self {
            result,
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            result: Value::Null,
            error: Some(message.into()),
        }
    }

    /// `{"result": ..., "error": ...}`, the shape fed back to the model.
    pub fn as_value(&self) -> Value {
        obj! { "result" => self.result.clone(), "error" => self.error.as_deref() }
    }
}

// ---------------------------------------------------------------------------
// Coercion
// ---------------------------------------------------------------------------

/// Argument-shape errors carry Python's `ValueError` text verbatim.
pub type ArgResult<T> = Result<T, String>;

fn coerce_str(name: &str, value: &Value) -> ArgResult<String> {
    match value {
        Value::Str(s) => Ok(s.clone()),
        other => Err(format!("{name}: expected a string, got {}", py_repr(other))),
    }
}

fn parse_py_int(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    let digits: String = trimmed.chars().filter(|c| *c != '_').collect();
    if trimmed.starts_with('_') || trimmed.ends_with('_') || trimmed.contains("__") {
        return None;
    }
    digits.parse::<i64>().ok()
}

fn coerce_int(name: &str, value: &Value) -> ArgResult<i64> {
    match value {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Int(i) => Ok(*i),
        Value::Float(f) if f.fract() == 0.0 && f.is_finite() => Ok(*f as i64),
        Value::Str(s) => parse_py_int(s)
            .ok_or_else(|| format!("{name}: expected an integer, got {}", py_repr(value))),
        other => Err(format!("{name}: expected an integer, got {}", py_repr(other))),
    }
}

fn coerce_bool(name: &str, value: &Value) -> ArgResult<bool> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::Int(0) => Ok(false),
        Value::Int(1) => Ok(true),
        Value::Str(s) => {
            let lowered = s.trim().to_lowercase();
            if TRUE_WORDS.contains(&lowered.as_str()) {
                Ok(true)
            } else if FALSE_WORDS.contains(&lowered.as_str()) {
                Ok(false)
            } else {
                Err(format!("{name}: expected a boolean, got {}", py_repr(value)))
            }
        }
        other => Err(format!("{name}: expected a boolean, got {}", py_repr(other))),
    }
}

/// `Optional[T]`: an explicit `null` stays `None`, a missing key takes the default.
fn optional<T>(
    name: &str,
    value: Option<&Value>,
    default: Option<T>,
    coerce: fn(&str, &Value) -> ArgResult<T>,
) -> ArgResult<Option<T>> {
    match value {
        None => Ok(default),
        Some(Value::Null) => Ok(None),
        Some(v) => coerce(name, v).map(Some),
    }
}

struct Args<'a> {
    class: &'a str,
    entries: &'a [(String, Value)],
}

impl<'a> Args<'a> {
    fn new(class: &'a str, raw: &'a Value, required: &[&str]) -> ArgResult<Self> {
        let entries: &[(String, Value)] = match raw {
            Value::Null => &[],
            Value::Object(entries) => entries,
            other => {
                return Err(format!(
                    "{class}: arguments must be an object, got {}",
                    py_type_name(other)
                ))
            }
        };
        let args = Self { class, entries };
        let missing: Vec<&str> = required
            .iter()
            .copied()
            .filter(|name| args.get(name).is_none())
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "{}: missing required argument(s): {}",
                args.class,
                missing.join(", ")
            ));
        }
        Ok(args)
    }

    fn get(&self, name: &str) -> Option<&'a Value> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    fn required_str(&self, name: &str) -> ArgResult<String> {
        coerce_str(name, self.get(name).expect("required keys were checked"))
    }

    fn str_or(&self, name: &str, default: &str) -> ArgResult<String> {
        self.get(name)
            .map_or_else(|| Ok(default.to_owned()), |v| coerce_str(name, v))
    }

    fn bool_or(&self, name: &str, default: bool) -> ArgResult<bool> {
        self.get(name)
            .map_or(Ok(default), |v| coerce_bool(name, v))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListFilesArgs {
    pub dir: String,
    pub recursive: bool,
}

impl ListFilesArgs {
    pub fn from_args(raw: &Value) -> ArgResult<Self> {
        let args = Args::new("ListFilesArgs", raw, &[])?;
        Ok(Self {
            dir: args.str_or("dir", ".")?,
            recursive: args.bool_or("recursive", false)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileArgs {
    pub path: String,
    pub start_line: Option<i64>,
    pub limit: Option<i64>,
}

impl ReadFileArgs {
    pub fn from_args(raw: &Value) -> ArgResult<Self> {
        let args = Args::new("ReadFileArgs", raw, &["path"])?;
        Ok(Self {
            path: args.required_str("path")?,
            start_line: optional("start_line", args.get("start_line"), None, coerce_int)?,
            limit: optional("limit", args.get("limit"), None, coerce_int)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCodeArgs {
    pub query: String,
    pub glob: Option<String>,
    /// Accepted for schema parity; the Python executor ignores it too.
    #[expect(dead_code, reason = "no semantic search backend exists yet")]
    pub semantic: bool,
    pub timeout: Option<i64>,
}

impl SearchCodeArgs {
    pub fn from_args(raw: &Value) -> ArgResult<Self> {
        let args = Args::new("SearchCodeArgs", raw, &["query"])?;
        Ok(Self {
            query: args.required_str("query")?,
            glob: optional("glob", args.get("glob"), None, coerce_str)?,
            semantic: args.bool_or("semantic", false)?,
            timeout: optional("timeout", args.get("timeout"), Some(30), coerce_int)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditFileArgs {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
}

impl EditFileArgs {
    pub fn from_args(raw: &Value) -> ArgResult<Self> {
        let args = Args::new("EditFileArgs", raw, &["path", "old_string", "new_string"])?;
        Ok(Self {
            path: args.required_str("path")?,
            old_string: args.required_str("old_string")?,
            new_string: args.required_str("new_string")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommandArgs {
    pub cmd: String,
    pub cwd: Option<String>,
    pub timeout: Option<i64>,
}

impl RunCommandArgs {
    pub fn new(cmd: impl Into<String>, timeout: Option<i64>) -> Self {
        Self {
            cmd: cmd.into(),
            cwd: None,
            timeout,
        }
    }

    pub fn from_args(raw: &Value) -> ArgResult<Self> {
        let args = Args::new("RunCommandArgs", raw, &["cmd"])?;
        Ok(Self {
            cmd: args.required_str("cmd")?,
            cwd: optional("cwd", args.get("cwd"), None, coerce_str)?,
            timeout: optional("timeout", args.get("timeout"), Some(120), coerce_int)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    #[test]
    fn lax_scalar_coercion() {
        let raw = parse(r#"{"cmd": "ls", "timeout": "120", "cwd": null, "extra": 1}"#).expect("json");
        let args = RunCommandArgs::from_args(&raw).expect("coerces");
        assert_eq!(args, RunCommandArgs { cmd: "ls".to_owned(), cwd: None, timeout: Some(120) });
        let raw = parse(r#"{"dir": "src", "recursive": "yes"}"#).expect("json");
        assert!(ListFilesArgs::from_args(&raw).expect("coerces").recursive);
        let raw = parse(r#"{"path": "a", "start_line": 2.0, "limit": true}"#).expect("json");
        let args = ReadFileArgs::from_args(&raw).expect("coerces");
        assert_eq!((args.start_line, args.limit), (Some(2), Some(1)));
        let raw = parse(r#"{"query": "x"}"#).expect("json");
        assert_eq!(SearchCodeArgs::from_args(&raw).expect("coerces").timeout, Some(30));
    }

    #[test]
    fn python_error_texts() {
        let raw = parse(r#"{"old_string": ""}"#).expect("json");
        assert_eq!(
            EditFileArgs::from_args(&raw).expect_err("missing"),
            "EditFileArgs: missing required argument(s): path, new_string"
        );
        let raw = parse(r#"{"path": 5}"#).expect("json");
        assert_eq!(
            ReadFileArgs::from_args(&raw).expect_err("type"),
            "path: expected a string, got 5"
        );
        let raw = parse(r#"{"cmd": "ls", "timeout": "soon"}"#).expect("json");
        assert_eq!(
            RunCommandArgs::from_args(&raw).expect_err("type"),
            "timeout: expected an integer, got 'soon'"
        );
        let raw = parse(r#"{"recursive": "maybe"}"#).expect("json");
        assert_eq!(
            ListFilesArgs::from_args(&raw).expect_err("type"),
            "recursive: expected a boolean, got 'maybe'"
        );
        assert_eq!(
            ListFilesArgs::from_args(&parse("[1]").expect("json")).expect_err("shape"),
            "ListFilesArgs: arguments must be an object, got list"
        );
        assert!(ListFilesArgs::from_args(&Value::Null).is_ok());
    }

    #[test]
    fn schemas_name_the_six_dispatched_tools() {
        let names: Vec<String> = openai_tool_schemas()
            .as_array()
            .expect("list")
            .iter()
            .map(|t| t.get("function").and_then(|f| f.get("name")).and_then(Value::as_str).expect("name").to_owned())
            .collect();
        assert_eq!(names, VALID_TOOLS);
    }
}
