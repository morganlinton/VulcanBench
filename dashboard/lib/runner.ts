// Client for the local run GUI routes (backend/runner/routes.py).
// These routes exist only when the backend runs with VULCANBENCH_RUNNER=1 and
// answer loopback clients only, so every helper here surfaces errors instead
// of falling back to empty data the way lib/api.ts does for the public pages.

import { API_BASE } from "@/lib/api";

export interface ContainerView {
  name: string;
  status: string;
  state: string;
  image: string;
}

export interface FirstCheck {
  status: "ok" | "warning" | "pending" | "fail" | "unknown";
  reasons: string[];
  requested_model: string | null;
  models: Record<string, number>;
  tool_calls: number;
  infra_hits: string[];
  rate_limit_status: string | null;
  evidence: string | null;
  window_open: boolean;
}

export type Reward = Record<string, number | string | null>;

export interface TrialView {
  id: string;
  name: string;
  status: string;
  reward: Reward | null;
  started_at: string | null;
  finished_at: string | null;
  elapsed_s: number | null;
  cost_usd: number | null;
  containers: ContainerView[];
  first_check: FirstCheck | null;
  log_sources: string[];
  exception: string | null;
}

export interface TrialRow {
  id: string;
  status: string;
  reward: Reward | null;
  elapsed_s: number | null;
}

export interface RunBase {
  id: string;
  kind: "gate" | "suite" | "task";
  name: string;
  status: string;
  owned: boolean;
  suite_or_task: string | null;
  model: string | null;
  harness: string | null;
  effort: string | null;
  source_tag: string | null;
  source_commit: string | null;
  source_dir?: string | null;
  attempts_done: number;
  attempts_total: number | null;
  solves: number | null;
  started_at: string | null;
  finished_at: string | null;
  elapsed_s: number | null;
  cost_usd: number | null;
  pid: number | null;
  path: string | null;
  first_check: FirstCheck | null;
  infra_retries: number | null;
  notes: string[];
  job: Record<string, unknown> | null;
}

export interface RunRow extends RunBase {
  trials: TrialRow[];
}

export interface RunView extends RunBase {
  trials: TrialView[];
  log_sources: string[];
}

export interface LogTail {
  run_id: string;
  trial: string | null;
  source: string;
  size: number;
  truncated: boolean;
  redacted: boolean;
  text: string;
}

export interface RunnerStatus {
  config: {
    enabled: boolean;
    home: string;
    gate_out: string;
    run_worktrees: string;
    run_roots: string[];
    first_check_after_s: number;
  };
  docker_available: boolean;
  gate_jobs_dir_exists: boolean;
  run_roots_present: string[];
  supervisor: string;
}

// A refusal from the runner. ``detail`` keeps the structured body when the
// backend sent one (``{code, message, ...}``, as the Suites routes do), so a
// page can react to ``code`` (``builder_locked``, ``gh_auth``) and not the text.
export class RunnerError extends Error {
  status: number;
  code: string | null;
  detail: Record<string, unknown> | null;
  constructor(status: number, message: string, detail: Record<string, unknown> | null = null) {
    super(message);
    this.status = status;
    this.detail = detail;
    this.code = detail && typeof detail.code === "string" ? detail.code : null;
  }
}

async function errorFrom(res: Response): Promise<RunnerError> {
  let message = res.statusText;
  let detail: Record<string, unknown> | null = null;
  try {
    const body = (await res.json()) as { detail?: unknown };
    if (typeof body.detail === "string") {
      message = body.detail;
    } else if (body.detail && typeof body.detail === "object" && !Array.isArray(body.detail)) {
      detail = body.detail as Record<string, unknown>;
      message = typeof detail.message === "string" ? detail.message : JSON.stringify(detail);
    } else if (body.detail) {
      message = JSON.stringify(body.detail);
    }
  } catch {
    // keep statusText
  }
  return new RunnerError(res.status, message, detail);
}

export async function runnerGet<T>(path: string): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, { cache: "no-store" });
  } catch {
    throw new RunnerError(0, `backend unreachable at ${API_BASE}`);
  }
  if (!res.ok) throw await errorFrom(res);
  return (await res.json()) as T;
}

export const ACTIVE_STATUSES = new Set([
  "queued",
  "waiting",
  "preparing",
  "running",
  "stopping",
  "external_running",
]);

export function isActive(status: string): boolean {
  return ACTIVE_STATUSES.has(status);
}

export function runHref(id: string): string {
  return `/runs/${encodeURIComponent(id)}`;
}

export function logPath(
  runId: string,
  source: string,
  trial: string | null,
  lines = 300,
): string {
  const qs = new URLSearchParams({ source, lines: String(lines) });
  if (trial) qs.set("trial", trial);
  return `/api/runner/runs/${encodeURIComponent(runId)}/log?${qs.toString()}`;
}

// Public suite names (CLAUDE.md): never "SWE v4".
const SUITE_NAMES: Record<string, string> = {
  "coding-intelligence-index-v4": "VulcanBench Frontier v4",
  "cii-v4": "VulcanBench Frontier v4",
  "routine-v1": "VulcanBench Routine v1",
  "routine-v2": "VulcanBench Routine v2",
  "conduct-v1": "VulcanBench Safety v1",
  "frontier-v5": "VulcanBench Frontier v5",
};

export function publicSuiteName(id: string | null): string | null {
  if (!id) return null;
  return SUITE_NAMES[id] ?? null;
}

// The suite's readable name: the public name of a VulcanBench suite, else the
// display name a custom suite's job recorded, else null (show the id).
export function suiteLabel(run: RunBase): string | null {
  const pub = publicSuiteName(run.suite_or_task);
  if (pub) return pub;
  const spec = (run.job?.spec ?? {}) as Record<string, unknown>;
  const name = typeof spec.suite_name === "string" ? spec.suite_name : "";
  return name && spec.suite === run.suite_or_task && name !== run.suite_or_task ? name : null;
}

// Author and validate jobs work on one task of a custom suite: their spec
// carries both ids, and the row names the suite as well as the task.
export function builderTarget(run: RunBase): { suite: string; suiteName: string; task: string } | null {
  if (!run.id.startsWith("job:") || !run.job) return null;
  const kind = String(run.kind);
  if (kind !== "author" && kind !== "validate") return null;
  const spec = (run.job.spec ?? {}) as Record<string, unknown>;
  const suite = typeof spec.suite === "string" ? spec.suite : "";
  const task = typeof spec.task === "string" ? spec.task : "";
  if (!suite || !task) return null;
  const suiteName = typeof spec.suite_name === "string" && spec.suite_name ? spec.suite_name : suite;
  return { suite, suiteName, task };
}

export function statusLabel(status: string): string {
  switch (status) {
    case "external_running":
      return "running";
    case "infra_failed":
      return "infra failure";
    default:
      return status.replace(/_/g, " ");
  }
}

export function fmtElapsed(s: number | null | undefined): string {
  if (s === null || s === undefined || !Number.isFinite(s)) return "n/a";
  const total = Math.max(0, Math.floor(s));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = total % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${String(sec).padStart(2, "0")}s`;
  return `${sec}s`;
}

export function fmtTime(iso: string | null | undefined): string {
  if (!iso) return "n/a";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function fmtMoney(v: number | null | undefined): string {
  if (v === null || v === undefined) return "n/a";
  return `$${v.toFixed(2)}`;
}

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function rewardValue(reward: Reward | null): number | null {
  if (!reward) return null;
  const r = reward.reward;
  if (typeof r === "number") return r;
  const f = reward.functional;
  if (typeof f === "number") return f >= 1 ? 1 : 0;
  return null;
}

// ----------------------------------------------------------------- launch --

export interface JobSpecSummary {
  argv: string[] | null;
  cwd: string | null;
  env: Record<string, string> | null;
  path_prepend: string | null;
  prep_argv: string[] | null;
  needs_prep: boolean | null;
  tag: string | null;
  worktree: string | null;
  estimate: Record<string, unknown> | null;
  warnings: string[] | null;
  sweep_id: string | null;
  sweep_name: string | null;
  model: string | null;
  harness: string | null;
  suite: string | null;
  suite_name: string | null;
  effort: string | null;
  out_dir: string | null;
  resume_at: string | null;
  quota_continue: { max: number; wait_s: number; used?: number } | null;
  // Suite builder jobs (kind author or validate).
  task?: string | null;
  task_dir?: string | null;
  pr?: number | null;
  batch_id?: string | null;
  billing?: string | null;
}

export interface JobSummary {
  id: number;
  kind: string;
  name: string;
  status: string;
  spec: JobSpecSummary;
  plan_hash: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  phase: string | null;
  pid: number | null;
  pgid: number | null;
  exit_code: number | null;
  stop_requested_at: string | null;
  stop_reason: string | null;
  stop_signalled_at: string | null;
  stop_escalated_at: string | null;
  console_log: string | null;
  notes: string[];
  jobs?: JobSummary[]; // every job a confirm created (one per effort level for a sweep)
}

export interface SupervisorInfo {
  status: "active" | "inactive" | "disabled" | "stopped";
  detail: string | null;
  last_tick?: string | null;
  last_error?: string | null;
  hold_reason?: string | null;
  children?: number[];
}

export interface QueueData {
  jobs: JobSummary[];
  supervisor: SupervisorInfo;
  outside: { pid: number; command: string }[];
}

export interface WorktreeInfo {
  tag: string;
  path: string;
  exists: boolean;
  tag_commit: string | null;
  head: string | null;
  at_tag: boolean;
  dirty: string[];
  problems: string[];
}

export interface SuiteInfo {
  id: string;
  name: string;
  version: string | null;
  frozen_at: string | null;
  task_ids: string[];
  n_tasks: number;
  /** True for a suite in the suites home, outside the checkout. */
  custom: boolean;
  path?: string;
  suites_home?: string;
  lock_digest?: string;
  repo?: { host?: string; owner?: string; name?: string; url?: string } | null;
}

export interface LaunchOptions {
  kinds: string[];
  tags: string[];
  tag: string | null;
  worktree: WorktreeInfo | null;
  run_worktrees: string;
  suites: SuiteInfo[];
  suites_home: string;
  harnesses: string[];
  efforts: string[];
  billing: string[];
  sandboxes: string[];
  sandbox_defaults: Record<string, string>;
  quota_wait_s: number;
  quota_continue_max: number;
  max_repeat: number;
  harness_runs_dir: string;
}

export interface DoctorResult {
  status: "ready" | "not_ready" | "not_applicable" | "unavailable" | "error";
  harness?: string | null;
  version?: string | null;
  auth_mode?: string | null;
  plan_name?: string | null;
  detail?: string | null;
  note?: string | null;
  exit_code?: number | null;
}

export interface SweepEstimate {
  status: "ok" | "no_pricing" | "unavailable";
  note: string | null;
  confidence: string | null;
  notes: string[];
  per_level_usd: number | null;
  per_level_mid_usd?: number | null;
  total_mid_usd?: number | null;
  per_level_low_usd: number | null;
  per_level_high_usd: number | null;
  total_usd: number | null;
  levels: number;
  runs_per_level: number;
  total_runs: number;
  task_timeout_s: number;
  max_wall_s: number;
  billing: string;
  infra_retries: string;
  quota: string;
}

export interface SweepJobSpec {
  kind: "suite";
  job_name: string;
  effort: string;
  argv: string[];
  cwd: string;
  env: Record<string, string>;
  out_dir: string;
  task_ids: string[];
  n_tasks: number;
}

export interface SweepPlanSpec {
  kind: "sweep";
  name: string;
  sweep_id: string;
  model: string;
  model_spec: string;
  harness: string;
  billing: string;
  suite: string;
  suite_name: string;
  suite_custom?: boolean;
  suites_home?: string | null;
  suite_lock_digest?: string | null;
  suite_path?: string | null;
  n_tasks: number;
  efforts: string[];
  repeat: number;
  only_missing: boolean;
  sandbox: string;
  sandbox_default: string;
  quota_continue: { max: number; wait_s: number };
  tag: string;
  tag_commit: string | null;
  worktree: string;
  cwd: string;
  env: Record<string, string>;
  path_prepend: string;
  env_passthrough: string[];
  out_root: string;
  jobs: SweepJobSpec[];
  doctor: DoctorResult;
  estimate: SweepEstimate;
  harness_features: { present: boolean; max_infra_retries: boolean; doctor: boolean };
  usage_check_required: boolean;
  warnings: string[];
  hash: string;
}

export interface PreparePlanSpec {
  kind: "prepare";
  job_name: string;
  tag: string;
  tag_commit: string | null;
  worktree: string;
  prep_argv: string[];
  prep_env: Record<string, string>;
  env_passthrough: string[];
  estimate: { billing: string; infra_retries: string };
  warnings: string[];
  hash: string;
}

export interface SweepProgress {
  out_dir: string;
  n_tasks: number;
  repeat: number;
  total_runs: number;
  runs_done: number;
  tasks_done: number;
  passed: number;
  pass_rate: number | null;
  cost_usd: number | null;
  avg_run_s: number | null;
  eta_s: number | null;
  current: {
    run_id: string;
    task_id: string;
    started_at: string | null;
    elapsed_s: number | null;
    quiet_s: number | null;
    path: string;
    budget_fraction?: number;
    run_id_href?: string | null;
  } | null;
  stalled: boolean;
  unfinished: number;
  missing: string[];
  complete: boolean;
}

export interface SweepLevel {
  job: JobSummary;
  progress: SweepProgress;
}

export interface SweepView {
  sweep_id: string;
  name: string;
  model: string | null;
  harness: string | null;
  suite: string | null;
  suite_name: string | null;
  tag: string | null;
  tag_commit: string | null;
  created_at: string;
  levels: SweepLevel[];
  totals: { runs_done: number; total_runs: number; passed: number; cost_usd: number | null; active: boolean };
}

export interface SweepsData {
  sweeps: SweepView[];
}

export interface OrphanContainer {
  id: string;
  name: string;
  status: string;
  state: string;
  image: string;
  project: string | null;
  run: string | null;
  reason: string;
}

export interface OrphanData {
  containers: OrphanContainer[];
  live_runs: string[];
  docker_available: boolean;
}

export interface TagPreflight {
  checkout: string;
  branch: string | null;
  head: string | null;
  origin_main: string | null;
  clean: boolean;
  problems: string[];
  script: string;
  argv_preview: string[];
}

export interface TagResult {
  ok: boolean;
  exit_code: number | null;
  output: string;
  tag: string | null;
  argv: string[];
}

export function logStreamPath(runId: string, source: string, trial: string | null, lines = 300): string {
  const qs = new URLSearchParams({ source, lines: String(lines) });
  if (trial) qs.set("trial", trial);
  return `${API_BASE}/api/runner/runs/${encodeURIComponent(runId)}/log/stream?${qs.toString()}`;
}

export type AnyPlanSpec = SweepPlanSpec | PreparePlanSpec;

export interface LaunchPlan<S = AnyPlanSpec> {
  id: string;
  hash: string;
  kind: string;
  spec: S;
  created_at: string;
  expires_at: string;
  confirmed_at: string | null;
  job_id: number | null;
  live_reason: string | null;
}

async function runnerSend<T>(method: string, path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, {
      method,
      cache: "no-store",
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new RunnerError(0, `backend unreachable at ${API_BASE}`);
  }
  if (!res.ok) throw await errorFrom(res);
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function runnerPost<T>(path: string, body?: unknown): Promise<T> {
  return runnerSend<T>("POST", path, body);
}

export function runnerDelete(path: string): Promise<void> {
  return runnerSend<void>("DELETE", path);
}

export function shellQuote(arg: string): string {
  return /^[A-Za-z0-9_./:=@%+,-]+$/.test(arg) ? arg : `'${arg.replace(/'/g, `'\\''`)}'`;
}

export function commandLine(env: Record<string, string> | null | undefined, argv: string[]): string {
  const prefix = Object.entries(env ?? {}).map(([k, v]) => `${k}=${shellQuote(v)}`);
  return [...prefix, ...argv.map(shellQuote)].join(" ");
}

export function fmtDuration(s: number): string {
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  if (h === 0 && s < 60) return `${Math.max(0, Math.round(s))} s`;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

export interface ReadinessCheck {
  id: string;
  label: string;
  status: "ok" | "warn" | "fail";
  detail: string;
  fix: string | null;
  needed_for: string;
}

export interface Readiness {
  status: "ok" | "warn" | "fail";
  checks: ReadinessCheck[];
}
