// Client for the VulcanBench backend (FastAPI, see backend/app.py).
// Override the base URL with NEXT_PUBLIC_API_BASE (defaults to localhost:8000).

export const API_BASE =
  process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

// GitHub repo used by the /submit flow (owner/name).
export const REPO =
  process.env.NEXT_PUBLIC_REPO ?? "Zen-Open-Source/VulcanBench";

export interface LeaderboardRow {
  run_id: string;
  task_id: string | null;
  model: string | null;
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

export function getLeaderboard(task?: string): Promise<LeaderboardRow[]> {
  const qs = task ? `?by=run&task=${encodeURIComponent(task)}` : "?by=run";
  return getJSON<LeaderboardRow[]>(`/api/leaderboard${qs}`, []);
}

export function getModelLeaderboard(suite?: string): Promise<ModelAggregate[]> {
  const qs = suite ? `?by=model&suite=${encodeURIComponent(suite)}` : "?by=model";
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
