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
