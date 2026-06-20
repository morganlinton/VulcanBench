// Client for the VulcanBench backend (FastAPI, see backend/app.py).
// Override the base URL with NEXT_PUBLIC_API_BASE (defaults to localhost:8000).

export const API_BASE =
  process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

// GitHub repo used by the /submit flow (owner/name).
export const REPO =
  process.env.NEXT_PUBLIC_REPO ?? "morganlinton/VulcanBench";

export interface LeaderboardRow {
  run_id: string;
  task_id: string | null;
  model: string | null;
  effort: EffortSummary | null;
  effort_requested: string | null;
  experiment_id: string | null;
  repo_scale: string | null;
  task_complexity: string | null;
  languages?: string[] | null;
  difficulty?: string | null;
  total: number | null;
  functional: number | null;
  steps: number | null;
  cost_usd: number | null;
  duration_s: number | null;
}

export interface ModelAggregate {
  model: string;
  n_tasks: number;
  n_runs: number;
  repeats: number;
  solved: number;
  pass_at_1: number | null;
  pass_at_1_stderr: number;
  pass_at_k: number | null;
  avg_total: number | null;
  avg_total_stderr: number;
  total_cost: number | null;
  cost_known: boolean;
  avg_duration_s: number | null;
}

export interface RunSummary {
  run_id: string;
  task_id: string;
  model: string;
  effort?: EffortSummary;
  experiment_id?: string;
  steps: number;
  total_tokens?: number;
  finished?: boolean;
  scores: Record<string, number | null>;
  replay_command?: string;
  artifacts?: Record<string, string>;
}

export interface TraceEvent {
  step: number;
  ts: string;
  type: string;
  data: Record<string, unknown>;
}

export interface TaskSummary {
  id: string;
  category: string | null;
  languages: string[];
  difficulty: string | null;
  task_complexity: string | null;
  source: string | null;
  empirical_difficulty: string | null;
  solve_rate: number | null;
  calibration_status: string | null;
}

export interface TaskDetail {
  id: string;
  metadata: Record<string, unknown>;
  issue: string;
  runs: LeaderboardRow[];
}

export interface CalibrationEntry {
  task_id: string;
  labeled_difficulty: string | null;
  n_attempts: number;
  n_models: number;
  models: string[];
  per_model_solve_rate: Record<string, number>;
  solve_rate: number | null;
  solve_rate_stderr: number | null;
  empirical_difficulty: string | null;
  status: string;
  agreement: boolean | null;
}

export interface Calibration {
  generated_at: string;
  thresholds: { easy_min: number; medium_min: number };
  criteria: { min_attempts: number; min_models: number };
  excluded: { mock_runs: number; stale_runs: number; unknown_task_runs: number };
  tasks: CalibrationEntry[];
  summary: { n_tasks: number; n_calibrated: number; n_disagree: number; n_insufficient: number };
}

export interface EffortSummary {
  requested: string;
  provider: string;
  provider_value: string | null;
  supported: boolean;
}

export interface EffortMetrics {
  n_runs: number;
  n_tasks: number;
  pass_at_1: number | null;
  avg_total: number | null;
  total_cost: number | null;
  cost_known: boolean;
  avg_duration_s: number | null;
  total_tokens: number;
}

export interface EffortStratum {
  model: string;
  language: string;
  repo_scale: string;
  difficulty: string;
  task_complexity: string;
  efforts: Record<string, EffortMetrics>;
  high_minus_low_pass_at_1: number | null;
  high_minus_low_avg_total: number | null;
  high_cost_ratio: number | null;
  high_latency_ratio: number | null;
  classification: string;
}

export interface EffortSensitivity {
  available: boolean;
  strata: EffortStratum[];
  warnings: string[];
}

// True when the backend answered the health check. Pages use this to
// distinguish "backend down" from genuinely empty data before rendering
// fallback values from getJSON.
export async function backendReachable(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/api/health`, { cache: "no-store" });
    return res.ok;
  } catch {
    return false;
  }
}

// Fetch JSON from the backend. Returns `fallback` if the backend is unreachable
// or responds with an error, so the dashboard degrades gracefully when run
// without the API (e.g. `npm run dev` alone).
async function getJSON<T>(path: string, fallback: T): Promise<T> {
  try {
    const res = await fetch(`${API_BASE}${path}`, { cache: "no-store" });
    if (!res.ok) return fallback;
    return (await res.json()) as T;
  } catch {
    return fallback;
  }
}

function leaderboardQuery(params: Record<string, string | undefined>): string {
  const qs = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value) qs.set(key, value);
  }
  const encoded = qs.toString();
  return encoded ? `?${encoded}` : "";
}

export function getLeaderboard(
  task?: string,
  effort?: string,
  taskComplexity?: string,
): Promise<LeaderboardRow[]> {
  const qs = leaderboardQuery({
    by: "run",
    task,
    effort,
    task_complexity: taskComplexity,
  });
  return getJSON<LeaderboardRow[]>(`/api/leaderboard${qs}`, []);
}

export function getModelLeaderboard(
  suite?: string,
  effort?: string,
  taskComplexity?: string,
): Promise<ModelAggregate[]> {
  const qs = leaderboardQuery({
    by: "model",
    suite,
    effort,
    task_complexity: taskComplexity,
  });
  return getJSON<ModelAggregate[]>(`/api/leaderboard${qs}`, []);
}

export function getRun(runId: string): Promise<RunSummary | null> {
  return getJSON<RunSummary | null>(`/api/run/${encodeURIComponent(runId)}`, null);
}

export function getTrace(runId: string): Promise<TraceEvent[]> {
  return getJSON<TraceEvent[]>(
    `/api/run/${encodeURIComponent(runId)}/trace`,
    [],
  );
}

export function getRunPatch(runId: string): Promise<string> {
  return getJSON<{ patch: string }>(
    `/api/run/${encodeURIComponent(runId)}/patch`,
    { patch: "" },
  ).then((r) => r.patch);
}

export function getTasks(): Promise<TaskSummary[]> {
  return getJSON<TaskSummary[]>(`/api/tasks`, []);
}

export function getTask(id: string): Promise<TaskDetail | null> {
  return getJSON<TaskDetail | null>(`/api/task/${encodeURIComponent(id)}`, null);
}

export function getCalibration(suite?: string): Promise<Calibration> {
  const qs = suite ? `?suite=${encodeURIComponent(suite)}` : "";
  return getJSON<Calibration>(
    `/api/calibration${qs}`,
    {
      generated_at: "",
      thresholds: { easy_min: 0.85, medium_min: 0.4 },
      criteria: { min_attempts: 5, min_models: 2 },
      excluded: { mock_runs: 0, stale_runs: 0, unknown_task_runs: 0 },
      tasks: [],
      summary: { n_tasks: 0, n_calibrated: 0, n_disagree: 0, n_insufficient: 0 },
    },
  );
}

export function getEffortSensitivity(suite?: string): Promise<EffortSensitivity> {
  const qs = suite ? `?suite=${encodeURIComponent(suite)}` : "";
  return getJSON<EffortSensitivity>(
    `/api/effort-sensitivity${qs}`,
    { available: false, strata: [], warnings: [] },
  );
}
