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

export class RunnerError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export async function runnerGet<T>(path: string): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, { cache: "no-store" });
  } catch {
    throw new RunnerError(0, `backend unreachable at ${API_BASE}`);
  }
  if (!res.ok) {
    let detail = res.statusText;
    try {
      const body = (await res.json()) as { detail?: string };
      if (body.detail) detail = body.detail;
    } catch {
      // keep statusText
    }
    throw new RunnerError(res.status, detail);
  }
  return (await res.json()) as T;
}

export const ACTIVE_STATUSES = new Set([
  "queued",
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
  prep_argv: string[] | null;
  needs_prep: boolean | null;
  tag: string | null;
  worktree: string | null;
  estimate: GateEstimate | null;
  warnings: string[] | null;
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

export interface GateRef {
  name: string;
  agent: string;
  model: string;
  version: string;
}

export interface WorktreeInfo {
  tag: string;
  path: string;
  exists: boolean;
  tag_commit: string | null;
  head: string | null;
  at_tag: boolean;
  dirty: string[];
  script: { present: boolean; status_file: boolean; preflight_only: boolean; refs: boolean };
  problems: string[];
}

export interface GateTask {
  name: string;
  language: string | null;
  default_concurrency: number;
}

export interface LaunchOptions {
  kinds: string[];
  refs: GateRef[];
  tags: string[];
  tag: string | null;
  worktree: WorktreeInfo | null;
  tasks: GateTask[];
  attempts: number[];
  concurrency: number[];
  gate_effort: string;
  credentials: { codex_auth: boolean; claude_gate_env: boolean };
  gate_out: string;
  run_worktrees: string;
}

export interface GateEstimate {
  attempts: number;
  concurrency: number;
  task_timeout_s: number;
  max_wall_s: number;
  billing: string;
  early_exit: string;
  infra_retries: string;
}

export interface UsageCheck {
  status: string;
  would_start: boolean;
  max_utilization?: number;
  windows?: Record<string, number>;
  max_allowed?: number;
  note?: string;
  exit_code?: number;
  model?: string;
}

export interface GatePlanSpec {
  kind: "gate";
  job_name: string;
  ref: string;
  agent: string;
  model: string;
  harness_version: string;
  effort: string;
  task: string;
  task_language: string | null;
  tag: string;
  tag_commit: string | null;
  worktree: string;
  needs_prep: boolean;
  attempts: number;
  concurrency: number;
  concurrency_overridden: boolean;
  suffix: string;
  argv: string[];
  cwd: string;
  env: Record<string, string>;
  env_passthrough: string[];
  prep_argv: string[] | null;
  prep_env: Record<string, string> | null;
  job_dir: string;
  status_file: string;
  script_features: WorktreeInfo["script"];
  estimate: GateEstimate;
  usage_check_required: boolean;
  warnings: string[];
  hash: string;
}

export interface LaunchPlan {
  id: string;
  hash: string;
  kind: string;
  spec: GatePlanSpec;
  created_at: string;
  expires_at: string;
  confirmed_at: string | null;
  job_id: number | null;
  usage_check: UsageCheck | null;
  usage_checked_at: string | null;
  live_reason: string | null;
  usage_check_ok: boolean;
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
  if (!res.ok) {
    let detail = res.statusText;
    try {
      const data = (await res.json()) as { detail?: unknown };
      if (typeof data.detail === "string") detail = data.detail;
      else if (data.detail) detail = JSON.stringify(data.detail);
    } catch {
      // keep statusText
    }
    throw new RunnerError(res.status, detail);
  }
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
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

export function fmtPercent(v: number | undefined): string {
  if (v === undefined || !Number.isFinite(v)) return "n/a";
  return `${Math.round(v * 100)}%`;
}
