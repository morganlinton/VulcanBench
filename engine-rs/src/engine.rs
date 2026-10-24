//! The run engine: `run_agent` and the ReAct step loop from
//! `harness/agent/loop.py`, for the mock and Anthropic providers in the local
//! or Docker sandbox.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::deadline::{budget_exhausted, tool_timeout, Deadline};
use crate::docker_executor::{
    docker_available, resolve_sandbox_image, DockerExecutor, ResourceSpec,
};
use crate::effort::{effort_config, EffortConfig};
use crate::error::EngineError;
use crate::evaluator::{budget_exceeded_scores, evaluate_run, EvaluateParams};
use crate::executor::Executor;
use crate::json::{obj, Value};
use crate::local_executor::LocalToolExecutor;
use crate::manifest::{
    collect_manifest, minimal_manifest, resolve_agent_timeout_s, resolve_max_steps,
    resolve_verifier_timeout_s, system_prompt_for_task, task_workspace_stats,
};
use crate::pricing::{cost_usd, is_priced};
use crate::process;
use crate::protocol::{
    openai_tool_schemas, RunCommandArgs, ToolCall, ToolObservation, VALID_TOOLS,
};
use crate::provenance::source_provenance;
use crate::provider::{get_provider, Provider, ToolInvocation};
use crate::pyfmt::{char_len, char_prefix, py_repr_str, py_round, py_str, utc_now_isoformat};
use crate::redaction::redact;
use crate::settings;
use crate::sha256::Sha256;
use crate::tasks::{load_task, prepare_workspace, task_hash, Task};
use crate::tracer::TraceCollector;
use crate::verifier::{run_declarative_verifier, run_verifier, RunnerOutcome, DEFAULT_TIMEOUT};

pub const SYSTEM_PROMPT: &str =
    "You are an autonomous software engineering agent. Solve the task described \
in the issue by using the provided tools to inspect and edit files and run \
tests. Make the smallest correct change. When the task is complete and tests \
pass, reply with a final message beginning with 'FINISH:' and stop calling \
tools.";

const FINISH_MARKER: &str = "FINISH:";

/// Cap on the serialized tool observation fed back to the model.
const MAX_OBSERVATION_CHARS: usize = 8_000;

const WORKSPACE_GITIGNORE: &str =
    ".coverage\n__pycache__/\n.pytest_cache/\n.ruff_cache/\n*.pyc\n.cursor/\n.grok/\n.zcode/\n\
.devin/\n\
target/\nnode_modules/\ndist/\nbuild/\n.gocache/\n.nyc_output/\n*.egg-info/\n\
*.o\n*.a\n*.dSYM/\nCMakeFiles/\nCMakeCache.txt\ncmake-build-*/\nzz_hidden_build/\n";

/// What the CLI asked for.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub task_id: String,
    pub model: String,
    pub output_dir: PathBuf,
    pub tasks_root: PathBuf,
    pub max_steps: Option<i64>,
    pub judges: bool,
    pub judge_model: Option<String>,
    pub sandbox: String,
    pub image: Option<String>,
    pub network: bool,
    pub resources: ResourceSpec,
    pub timeout_s: Option<f64>,
    pub effort: Option<String>,
    pub max_run_cost: Option<f64>,
    pub override_budgets: bool,
}

/// What a finished run hands back to the CLI.
#[derive(Debug, Clone)]
pub struct RunResult {
    pub run_id: String,
    pub summary: Value,
}

struct LoopOutcome {
    prompt_tokens: i64,
    completion_tokens: i64,
    finished: bool,
    cost_capped: bool,
    actual_steps: i64,
}

struct RunContext<'a> {
    collector: &'a mut TraceCollector,
    executor: &'a Executor,
    deadline: &'a mut Deadline,
}

/// Eight hex characters standing in for `uuid4().hex[:8]`.
fn short_hex() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let marker = 0u8;
    let mut hasher = Sha256::new();
    hasher.update(&nanos.to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    hasher.update(&(std::ptr::addr_of!(marker) as usize).to_le_bytes());
    hasher.hexdigest()[..8].to_owned()
}

/// Runs one evaluation: the agent solves `task_id` with `model`.
///
/// # Errors
///
/// Every failure the Python CLI maps to an exit code: blocked effort, unknown
/// task or provider, sandbox failures, provider failures, verifier
/// infrastructure, I/O.
pub fn run_agent(opts: &RunOptions) -> Result<RunResult, EngineError> {
    settings::check_effort_allowed(opts.effort.as_deref(), &settings::settings_path())?;
    let task = load_task(&opts.task_id, &opts.tasks_root)?;
    let provider = get_provider(&opts.model)?;
    let effort_meta = effort_config(provider.name(), opts.effort.as_deref())?;
    if opts.judges {
        return Err(EngineError::Invalid(
            "the human_like judge ensemble is not implemented in the Rust spike; pass --no-judges"
                .to_owned(),
        ));
    }
    let grader = task
        .metadata
        .get("grader")
        .map(py_str)
        .unwrap_or_else(|| "tests".to_owned());
    if grader != "tests" {
        return Err(EngineError::Invalid(format!(
            "the {grader} grader is not implemented in the Rust spike"
        )));
    }
    if task.metadata.get("environment").is_some() {
        return Err(EngineError::Sandbox(
            "multi-service task environments are not implemented in the Rust spike".to_owned(),
        ));
    }
    let effective_max_steps =
        resolve_max_steps(&task.metadata, opts.max_steps, opts.override_budgets);
    let effective_timeout =
        resolve_agent_timeout_s(&task.metadata, opts.timeout_s, opts.override_budgets);

    let run_id = format!("{}-{}", task.task_id, short_hex());
    let run_dir = opts.output_dir.join(&run_id);
    let mut collector = TraceCollector::new(&run_dir, &run_id, &task.task_id, &opts.model)?;
    collector.record(
        "task_start",
        obj! { "task_id" => task.task_id.as_str(), "metadata" => task.metadata.clone() },
    )?;

    let workspace = run_dir.join("workspace");
    prepare_workspace(&task, &workspace)?;
    git_init(&workspace)?;
    let executor = make_executor(opts, &task, &workspace, &mut collector)?;
    run_task_setup(&task, &executor, &mut collector)?;

    let started_at = utc_now_isoformat();
    let mut deadline = Deadline::new(effective_timeout);
    let tools = openai_tool_schemas();
    let mut messages = vec![
        obj! { "role" => "system", "content" => system_prompt_for_task(&task.metadata, SYSTEM_PROMPT) },
        obj! { "role" => "user", "content" => format!("# Issue\n\n{}", task.issue) },
    ];

    let mut ctx = RunContext {
        collector: &mut collector,
        executor: &executor,
        deadline: &mut deadline,
    };
    let effort_summary = effort_meta.as_ref().map(EffortConfig::as_summary);
    let provider_effort = effort_meta
        .as_ref()
        .filter(|e| e.supported)
        .and_then(|e| e.provider_value.clone());
    let loop_inputs = LoopInputs {
        provider: provider.as_ref(),
        tools: &tools,
        max_steps: effective_max_steps,
        effort_summary: effort_summary.as_ref(),
        provider_effort: provider_effort.as_deref(),
        model: &opts.model,
        max_run_cost: opts.max_run_cost,
    };
    let outcome = run_model_loop(&mut ctx, &loop_inputs, &mut messages)?;

    let patch = git_diff(&workspace)?;
    let changed_files = git_changed_files(&workspace)?;
    // The patch is an integrity artifact: redacted, never length-capped.
    fs::write(run_dir.join("final.patch"), redact(&patch))
        .map_err(|e| EngineError::io("writing final.patch", e))?;
    ctx.collector
        .record("diff", obj! { "patch" => patch.as_str() })?;

    let (functional, verifier_payload) = verify_with_budget(&task, &mut ctx)?;
    let manifest = collect_manifest_with_budget(opts, &task, &mut ctx)?;
    // Quality, security and judges run host-side over the mounted workspace,
    // so the container comes down now, as in the Python engine's `finally`.
    executor.close();

    let total_tokens = outcome.prompt_tokens + outcome.completion_tokens;
    let scores = evaluate_with_budget(
        &mut ctx,
        &EvaluateParams {
            functional,
            total_tokens,
            steps: outcome.actual_steps,
            workspace: &workspace,
            changed_files: &changed_files,
            judges_enabled: opts.judges,
        },
    )?;
    ctx.collector.record("metric_computed", scores.clone())?;

    let cost = compute_cost(
        &opts.model,
        outcome.prompt_tokens,
        outcome.completion_tokens,
    );
    let economics = api_receipt(
        cost.get("total").and_then(Value::as_f64),
        cost.get("judges").and_then(Value::as_f64),
    );
    let duration_s = py_round(ctx.deadline.elapsed_s(), 3);
    let mut extra = obj! {
        "steps" => outcome.actual_steps,
        "total_tokens" => total_tokens,
        "tokens" => obj! {
            "prompt" => outcome.prompt_tokens,
            "completion" => outcome.completion_tokens,
            "total" => total_tokens,
        },
        "cost_usd" => cost.get("total").cloned().unwrap_or(Value::Null),
        "cost_detail" => cost,
        "economics" => economics,
        "duration_s" => duration_s,
        "started_at" => started_at,
        "finished" => outcome.finished,
        "cost_capped" => outcome.cost_capped,
        "manifest" => manifest,
        "task_hash" => task_hash(&task)?,
        "source" => source_provenance().as_summary(),
        "suite" => Value::Null,
        "suite_id" => Value::Null,
    };
    if let Some(effort) = effort_summary {
        extra.insert("effort", effort);
    }
    if opts.override_budgets {
        extra.insert(
            "budget_override",
            obj! { "timeout_s" => effective_timeout, "max_steps" => effective_max_steps },
        );
    }
    extra.insert("verifier", verifier_payload);
    extra.insert("replay_command", format!("vulcanbench replay {run_id}"));
    let summary = collector.finalize(scores, extra)?;
    // replay.html is out of scope for the spike; the Python engine renders it here.
    if std::env::var_os("VULCANBENCH_API_BASE").is_some() {
        eprintln!("[vulcanbench-engine] VULCANBENCH_API_BASE is set but the spike has no backend client; write-through skipped");
    }
    Ok(RunResult { run_id, summary })
}

/// `_make_executor`: the sandbox for the requested mode. `docker` fails when
/// the daemon is unavailable; `auto` falls back to the host only with the
/// explicit `VULCANBENCH_ALLOW_HOST_EXEC=1` opt-in, loudly.
fn make_executor(
    opts: &RunOptions,
    task: &Task,
    workspace: &Path,
    collector: &mut TraceCollector,
) -> Result<Executor, EngineError> {
    let resolved_image = resolve_sandbox_image(task, opts.image.as_deref());
    let docker = |collector: &mut TraceCollector| -> Result<Executor, EngineError> {
        collector.record(
            "sandbox",
            obj! {
                "mode" => "docker",
                "image" => resolved_image.as_str(),
                "network" => opts.network,
                "resources" => opts.resources.to_dict(),
            },
        )?;
        Ok(Executor::Docker(DockerExecutor::new(
            workspace,
            &resolved_image,
            opts.network,
            opts.resources.clone(),
        )?))
    };
    match opts.sandbox.as_str() {
        "local" => {
            collector.record("sandbox", obj! { "mode" => "local" })?;
            Ok(Executor::Local(LocalToolExecutor::new(workspace)?))
        }
        "docker" => docker(collector),
        "auto" => {
            if docker_available() {
                return docker(collector);
            }
            if std::env::var("VULCANBENCH_ALLOW_HOST_EXEC").ok().as_deref() != Some("1") {
                return Err(EngineError::Sandbox(
                    "sandbox 'auto': Docker daemon unavailable. Refusing to run \
                     model-written commands on the host. Start Docker, pass \
                     --sandbox local, or set VULCANBENCH_ALLOW_HOST_EXEC=1 to allow \
                     the fallback."
                        .to_owned(),
                ));
            }
            collector.record(
                "sandbox_fallback",
                obj! { "requested" => "auto", "using" => "local", "reason" => "docker daemon unavailable" },
            )?;
            eprintln!("[vulcanbench] WARNING: Docker daemon unavailable; falling back to local (host) execution.");
            Ok(Executor::Local(LocalToolExecutor::new(workspace)?))
        }
        other => Err(EngineError::Invalid(format!(
            "unknown sandbox mode {}; expected local|docker|auto",
            py_repr_str(other)
        ))),
    }
}

struct LoopInputs<'a> {
    provider: &'a dyn Provider,
    tools: &'a Value,
    max_steps: i64,
    effort_summary: Option<&'a Value>,
    provider_effort: Option<&'a str>,
    model: &'a str,
    max_run_cost: Option<f64>,
}

fn run_model_loop(
    ctx: &mut RunContext<'_>,
    inputs: &LoopInputs<'_>,
    messages: &mut Vec<Value>,
) -> Result<LoopOutcome, EngineError> {
    let mut outcome = LoopOutcome {
        prompt_tokens: 0,
        completion_tokens: 0,
        finished: false,
        cost_capped: false,
        actual_steps: 0,
    };
    for step in 1..=inputs.max_steps {
        if !ctx
            .deadline
            .ensure_time(ctx.collector, "llm_request", Some(step))?
        {
            break;
        }
        outcome.actual_steps = step;
        let mut request = obj! { "step" => step, "messages" => messages.len() };
        if let Some(effort) = inputs.effort_summary {
            request.insert("effort", effort.clone());
        }
        ctx.collector.record("llm_request", request)?;
        let resp = match inputs.provider.complete(
            messages,
            inputs.tools,
            ctx.deadline.remaining_s(),
            inputs.provider_effort,
        ) {
            Ok(resp) => resp,
            Err(err) => {
                if ctx.deadline.is_expired() {
                    ctx.deadline
                        .record_exceeded(ctx.collector, "llm_request", Some(step))?;
                    break;
                }
                return Err(err);
            }
        };
        outcome.prompt_tokens += resp.usage.prompt_tokens;
        outcome.completion_tokens += resp.usage.completion_tokens;
        let tool_calls: Vec<Value> = resp
            .tool_calls
            .iter()
            .map(ToolInvocation::to_dict)
            .collect();
        ctx.collector.record(
            "llm_response",
            obj! {
                "content" => resp.content.as_deref(),
                "tool_calls" => tool_calls,
                "usage" => resp.usage.to_dict(),
            },
        )?;
        if !ctx
            .deadline
            .ensure_time(ctx.collector, "llm_response", Some(step))?
        {
            break;
        }
        // Per-run cost ceiling: stop before paying for another call once this
        // run's own spend crosses the cap. Only enforceable for priced models.
        if let Some(cap) = inputs.max_run_cost {
            if let Some(run_cost) = cost_usd(
                inputs.model,
                outcome.prompt_tokens,
                outcome.completion_tokens,
                0,
            ) {
                if run_cost >= cap {
                    ctx.collector.record(
                        "cost_cap_exceeded",
                        obj! { "cost_usd" => run_cost, "max_run_cost" => cap, "step" => step },
                    )?;
                    outcome.cost_capped = true;
                    messages.push(assistant_message(resp.content.as_deref(), &resp.tool_calls));
                    break;
                }
            }
        }
        messages.push(assistant_message(resp.content.as_deref(), &resp.tool_calls));
        if !resp.wants_tools() {
            outcome.finished = resp
                .content
                .as_deref()
                .is_some_and(|c| c.contains(FINISH_MARKER));
            break;
        }
        for tc in &resp.tool_calls {
            let stage = format!("tool:{}", tc.name);
            if !ctx
                .deadline
                .ensure_time(ctx.collector, &stage, Some(step))?
            {
                break;
            }
            let observation = execute_tool(ctx.executor, tc, ctx.deadline.remaining_s());
            ctx.collector.record(
                "tool_call",
                obj! { "tool" => tc.name.as_str(), "args" => tc.arguments.clone(), "id" => tc.id.as_str() },
            )?;
            ctx.collector.record(
                "tool_observation",
                obj! {
                    "tool" => tc.name.as_str(),
                    "result" => observation.result.clone(),
                    "error" => observation.error.as_deref(),
                },
            )?;
            messages.push(obj! {
                "role" => "tool",
                "tool_call_id" => tc.id.as_str(),
                "content" => truncate_observation(observation.as_value().to_json()),
            });
            if !ctx
                .deadline
                .ensure_time(ctx.collector, &stage, Some(step))?
            {
                break;
            }
        }
    }
    Ok(outcome)
}

/// Caps the serialized observation fed back to the model, with an explicit
/// marker so the model knows it saw a partial result (`len()` counts code points).
fn truncate_observation(content: String) -> String {
    let length = char_len(&content);
    if length <= MAX_OBSERVATION_CHARS {
        return content;
    }
    let omitted = length - MAX_OBSERVATION_CHARS;
    format!(
        "{}...[output truncated: {omitted} chars omitted]",
        char_prefix(&content, MAX_OBSERVATION_CHARS)
    )
}

fn assistant_message(content: Option<&str>, tool_calls: &[ToolInvocation]) -> Value {
    let mut message = obj! { "role" => "assistant", "content" => content.unwrap_or_default() };
    if !tool_calls.is_empty() {
        let calls: Vec<Value> = tool_calls
            .iter()
            .enumerate()
            .map(|(i, tc)| {
                let id = if tc.id.is_empty() { format!("call_{i}") } else { tc.id.clone() };
                obj! {
                    "id" => id,
                    "type" => "function",
                    "function" => obj! { "name" => tc.name.as_str(), "arguments" => tc.arguments.to_json() },
                }
            })
            .collect();
        message.insert("tool_calls", calls);
    }
    message
}

/// `int(requested)` as `_execute_run_command_tool` applies it, `None` on failure.
fn requested_timeout(value: &Value) -> Option<i64> {
    match value {
        Value::Int(i) => Some(*i),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Float(f) if f.is_finite() => Some(f.trunc() as i64),
        Value::Str(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn execute_tool(
    executor: &Executor,
    tc: &ToolInvocation,
    remaining: Option<f64>,
) -> ToolObservation {
    if !VALID_TOOLS.contains(&tc.name.as_str()) {
        return ToolObservation::err(format!("unknown tool {}", py_repr_str(&tc.name)));
    }
    if budget_exhausted(remaining) {
        return ToolObservation::err("run budget exceeded");
    }
    match tc.name.as_str() {
        "run_command" | "search_code" => {
            let mut args = tc.arguments.clone();
            if let Some(cap) = tool_timeout(remaining) {
                let requested = args
                    .get("timeout")
                    .filter(|v| !v.is_null())
                    .and_then(requested_timeout)
                    .unwrap_or(cap);
                if !matches!(args, Value::Object(_)) {
                    args = Value::object();
                }
                args.insert("timeout", requested.min(cap));
            }
            executor.execute(&ToolCall {
                tool: tc.name.clone(),
                args,
            })
        }
        "run_tests" => {
            let args = RunCommandArgs::new(
                "python -m pytest -q --tb=no || true",
                tool_timeout(remaining),
            );
            match executor.run_command(&args) {
                Ok(result) => ToolObservation::ok(result),
                Err(message) => ToolObservation::err(message),
            }
        }
        _ => executor.execute(&ToolCall {
            tool: tc.name.clone(),
            args: tc.arguments.clone(),
        }),
    }
}

fn run_task_setup(
    task: &Task,
    executor: &Executor,
    collector: &mut TraceCollector,
) -> Result<(), EngineError> {
    let timeout = i64::try_from(task.setup_timeout_s()).unwrap_or(600);
    for (name, cmd) in task.setup_commands() {
        let started = std::time::Instant::now();
        let exit_code = runner_outcome(executor, &cmd, timeout)?.exit_code;
        let duration = py_round(started.elapsed().as_secs_f64(), 3);
        collector.record(
            "setup",
            obj! { "name" => name.as_str(), "cmd" => cmd.as_str(), "exit_code" => exit_code, "duration_s" => duration },
        )?;
        if exit_code != 0 {
            return Err(EngineError::Invalid(format!(
                "setup command {} failed (exit code {exit_code}): {cmd}",
                py_repr_str(&name)
            )));
        }
    }
    Ok(())
}

/// `_executor_runner_outcome`: one verifier command through the executor. An
/// OOM kill at the sandbox ceiling is an infrastructure event, never a verdict.
fn runner_outcome(
    executor: &Executor,
    cmd: &str,
    timeout: i64,
) -> Result<RunnerOutcome, EngineError> {
    match executor.run_command(&RunCommandArgs::new(cmd, Some(timeout))) {
        Err(message) => Ok(RunnerOutcome {
            exit_code: 124,
            stdout: String::new(),
            stderr: message,
        }),
        Ok(res) => {
            let oom = res.get("oom_killed").and_then(Value::as_bool) == Some(true);
            if let (true, Some(ceiling)) = (oom, executor.mem_ceiling()) {
                return Err(EngineError::VerifierInfrastructure(format!(
                    "verifier command OOM-killed at the sandbox memory ceiling ({ceiling}): {}",
                    py_repr_str(cmd)
                )));
            }
            let non_empty = |key: &str| res.get(key).map(py_str).filter(|s| !s.is_empty());
            Ok(RunnerOutcome {
                exit_code: res.get("exit_code").and_then(Value::as_i64).unwrap_or(1),
                stdout: non_empty("stdout")
                    .or_else(|| non_empty("result"))
                    .unwrap_or_default(),
                stderr: non_empty("stderr").unwrap_or_default(),
            })
        }
    }
}

fn verify_with_budget(task: &Task, ctx: &mut RunContext<'_>) -> Result<(f64, Value), EngineError> {
    if !ctx.deadline.ensure_time(ctx.collector, "verify", None)? {
        return Ok((
            0.0,
            obj! {
                "scores" => obj! { "functional" => 0.0, "error" => "run budget exceeded before verification" },
                "budget_exceeded" => true,
            },
        ));
    }
    let verifier_budget = resolve_verifier_timeout_s(&task.metadata, DEFAULT_TIMEOUT);
    let timeout = ctx
        .deadline
        .timeout_for(verifier_budget)
        .filter(|t| *t != 0)
        .unwrap_or(1);
    let workspace = ctx.executor.workspace();
    let payload = if task.tests_spec().is_some() {
        let executor = ctx.executor;
        let runner = move |cmd: &str, timeout: i64| runner_outcome(executor, cmd, timeout);
        run_declarative_verifier(task, workspace, &runner, timeout)?
    } else if let Some(verifier) = &task.verifier {
        run_verifier(verifier, workspace, timeout)
    } else {
        return Ok((0.0, obj! { "error" => "no verifier for task" }));
    };
    ctx.collector.record("test_result", payload.clone())?;
    let functional = payload
        .get("scores")
        .and_then(|s| s.get("functional"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if ctx.deadline.is_expired() {
        ctx.deadline
            .record_exceeded(ctx.collector, "verify", None)?;
    }
    Ok((functional, payload))
}

fn collect_manifest_with_budget(
    opts: &RunOptions,
    task: &Task,
    ctx: &mut RunContext<'_>,
) -> Result<Value, EngineError> {
    let judge_model = opts.judge_model.as_deref();
    let workspace = ctx.executor.workspace();
    if !ctx.deadline.ensure_time(ctx.collector, "manifest", None)? {
        let mut manifest = minimal_manifest(&opts.model, judge_model, opts.network, ctx.executor);
        manifest.extend(task_workspace_stats(task, workspace));
        return Ok(manifest);
    }
    let mut manifest = collect_manifest(
        &opts.model,
        judge_model,
        opts.network,
        ctx.executor,
        ctx.deadline,
    );
    manifest.extend(task_workspace_stats(task, workspace));
    if ctx.deadline.is_expired() {
        ctx.deadline
            .record_exceeded(ctx.collector, "manifest", None)?;
    }
    Ok(manifest)
}

fn evaluate_with_budget(
    ctx: &mut RunContext<'_>,
    params: &EvaluateParams<'_>,
) -> Result<Value, EngineError> {
    if !ctx.deadline.ensure_time(ctx.collector, "evaluate", None)? {
        return Ok(budget_exceeded_scores(
            params.functional,
            params.total_tokens,
            params.steps,
        ));
    }
    let mut scores = evaluate_run(params, ctx.collector, ctx.deadline)?;
    if ctx.deadline.exceeded {
        scores.insert("budget_exceeded", true);
    }
    Ok(scores)
}

/// `_compute_cost`: agent plus judge cost; `total` is `None` for unpriced models.
fn compute_cost(model: &str, prompt_tokens: i64, completion_tokens: i64) -> Value {
    let agent = cost_usd(model, prompt_tokens, completion_tokens, 0);
    let judges = 0.0;
    let total = agent.map(|a| py_round(a + judges, 6));
    obj! {
        "agent" => agent,
        "judges" => judges,
        "total" => total,
        "model_priced" => is_priced(model),
        "agent_input_tokens" => prompt_tokens,
        "agent_cached_input_tokens" => 0,
        "agent_uncached_input_tokens" => prompt_tokens.max(0),
        "cache_pricing_applied" => false,
    }
}

/// `EconomicsReceipt.as_summary()` for a metered API run.
fn api_receipt(total_cost_usd: Option<f64>, grading_cost_usd: Option<f64>) -> Value {
    let quality = if total_cost_usd.is_some() {
        "provider-priced"
    } else {
        "unavailable"
    };
    obj! {
        "billing_mode" => "api-metered",
        "cost_basis" => "metered-api-pricing",
        "marginal_cash_usd" => total_cost_usd,
        "overage_cash_usd" => Value::Null,
        "grading_cash_usd" => grading_cost_usd,
        "grading_api_equivalent_usd" => grading_cost_usd,
        "plan_name" => Value::Null,
        "plan_fee_usd" => Value::Null,
        "allocated_plan_cost_usd" => Value::Null,
        "api_equivalent_cost_usd" => total_cost_usd,
        "cli_reported_cost_usd" => Value::Null,
        "quota" => Value::Null,
        "measurement_quality" => obj! {
            "marginal_cash_usd" => quality,
            "api_equivalent_cost_usd" => quality,
        },
    }
}

fn git(workspace: &Path, args: &[&str], author_env: bool) -> Result<String, EngineError> {
    let mut command = Command::new("git");
    command.args(args).current_dir(workspace);
    if author_env {
        command
            .env("GIT_AUTHOR_NAME", "vulcanbench")
            .env("GIT_AUTHOR_EMAIL", "bot@vulcanbench")
            .env("GIT_COMMITTER_NAME", "vulcanbench")
            .env("GIT_COMMITTER_EMAIL", "bot@vulcanbench");
    }
    let done = process::run(&mut command, Some(Duration::from_secs(600)))
        .map_err(|e| EngineError::io(format!("running git {}", args.join(" ")), e))?;
    Ok(done.stdout)
}

/// A throwaway git repo so the agent's changes can be diffed.
fn git_init(workspace: &Path) -> Result<(), EngineError> {
    fs::write(workspace.join(".gitignore"), WORKSPACE_GITIGNORE)
        .map_err(|e| EngineError::io("writing workspace .gitignore", e))?;
    git(workspace, &["init", "-q"], true)?;
    git(workspace, &["add", "-A"], true)?;
    git(
        workspace,
        &["commit", "-q", "--allow-empty", "-m", "base"],
        true,
    )?;
    Ok(())
}

fn git_diff(workspace: &Path) -> Result<String, EngineError> {
    git(workspace, &["add", "-A"], false)?;
    git(workspace, &["diff", "--cached"], false)
}

fn git_changed_files(workspace: &Path) -> Result<Vec<String>, EngineError> {
    git(workspace, &["add", "-A"], false)?;
    let out = git(workspace, &["diff", "--cached", "--name-only"], false)?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_and_receipt_shapes_for_mock() {
        let cost = compute_cost("mock:synthetic", 320, 80);
        assert_eq!(
            cost.to_json(),
            r#"{"agent": 0.0, "judges": 0.0, "total": 0.0, "model_priced": true, "agent_input_tokens": 320, "agent_cached_input_tokens": 0, "agent_uncached_input_tokens": 320, "cache_pricing_applied": false}"#
        );
        let receipt = api_receipt(Some(0.0), Some(0.0));
        assert_eq!(
            receipt.get("billing_mode"),
            Some(&Value::Str("api-metered".to_owned()))
        );
        assert_eq!(
            receipt.get("measurement_quality").map(Value::to_json),
            Some(r#"{"marginal_cash_usd": "provider-priced", "api_equivalent_cost_usd": "provider-priced"}"#.to_owned())
        );
        assert!(compute_cost("openai:x", 1, 1)
            .get("total")
            .is_some_and(Value::is_null));
        let priced = compute_cost("anthropic:claude-opus-5-5", 1_000_000, 100_000);
        assert_eq!(priced.get("total"), Some(&Value::Float(6.0)));
        assert_eq!(priced.get("model_priced"), Some(&Value::Bool(true)));
    }

    #[test]
    fn assistant_message_and_timeouts() {
        let calls = vec![ToolInvocation {
            id: String::new(),
            name: "read_file".to_owned(),
            arguments: obj! { "path" => "a" },
        }];
        let message = assistant_message(None, &calls);
        assert_eq!(
            message.to_json(),
            r#"{"role": "assistant", "content": "", "tool_calls": [{"id": "call_0", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\": \"a\"}"}}]}"#
        );
        let long = "é".repeat(MAX_OBSERVATION_CHARS + 3);
        let truncated = truncate_observation(long);
        assert!(truncated.ends_with("...[output truncated: 3 chars omitted]"));
        assert_eq!(
            char_len(&truncated),
            MAX_OBSERVATION_CHARS + "...[output truncated: 3 chars omitted]".len()
        );
        assert_eq!(truncate_observation("short".to_owned()), "short");
        assert_eq!(requested_timeout(&Value::Str(" 7 ".to_owned())), Some(7));
        assert_eq!(requested_timeout(&Value::Float(9.9)), Some(9));
        assert_eq!(requested_timeout(&Value::Str("soon".to_owned())), None);
        assert_eq!(short_hex().len(), 8);
    }
}
