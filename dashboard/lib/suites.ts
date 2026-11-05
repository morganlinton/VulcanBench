// Client for the suite builder routes (backend/runner/routes_suites.py).
// Every change to a suite runs `vulcanbench suite ...` in the tooling worktree;
// the engine's password gate answers 403 with code builder_locked,
// builder_not_configured or wrong_password, which the pages turn into the
// unlock screen.

import { RunnerError, runnerGet, runnerPost } from "@/lib/runner";

export const SUITES_API = "/api/runner/suites";

export interface Tooling {
  tag: string | null;
  worktree: string | null;
  tag_commit: string | null;
  ready: boolean;
  problems: string[];
}

export interface Access {
  configured: boolean;
  unlocked: boolean;
  unlocked_at: string | null;
}

export interface AccessResponse {
  tooling: Tooling;
  access: Access | null;
}

export interface RepoRef {
  host?: string;
  owner: string;
  name: string;
  url?: string;
}

export interface JobRef {
  id: number;
  kind: "author" | "validate";
  status: string;
  run_id: string;
}

export type Stage =
  | "missing"
  | "scaffolded"
  | "authoring"
  | "authored"
  | "author_failed"
  | "validating"
  | "validated"
  | "validation_failed";

export interface SuiteRow {
  id: string;
  name: string;
  repo: RepoRef | null;
  status: "draft" | "frozen";
  path: string;
  version: string | null;
  frozen_at: string | null;
  lock_digest: string | null;
  n_accepted: number;
  n_tasks: number;
  n_dropped: number;
  stages: Partial<Record<Stage, number>>;
  active_job: JobRef | null;
  updated_at: string | null;
}

export interface SuitesList {
  suites_home: string;
  tooling: Tooling;
  suites: SuiteRow[];
}

export interface TaskRow {
  id: string;
  pr: number | null;
  title: string | null;
  stage: Stage;
  accepted: boolean;
  accepted_unvalidated: boolean;
  dropped: boolean;
  dropped_reason: string | null;
  scaffold: {
    base_commit: string | null;
    languages: string[] | null;
    image: string | null;
    gold_applies: boolean | null;
    warnings: string[];
    scaffolded_at: string | null;
  } | null;
  author: {
    status: string | null;
    harness: string | null;
    model: string | null;
    effort: string | null;
    finished_at: string | null;
    duration_s: number | null;
    cost_usd: number | null;
    warnings: string[];
    accepted: number;
    rejected: number;
  } | null;
  validate: {
    status: string | null;
    reasons: string[];
    checks: Record<string, boolean | null>;
    sandbox: string | null;
    finished_at: string | null;
  } | null;
  /** A validation exists but is older than the last authoring session, so it no longer counts. */
  validate_stale: boolean;
  active_job: JobRef | null;
  last_job_id: number | null;
}

export interface SuiteDetail {
  suite: SuiteRow;
  tasks: TaskRow[];
  tooling: Tooling;
  freeze_ready: { ok: boolean; problems: string[] };
  options: { harnesses: string[]; efforts: string[]; billing: string[] };
}

export interface PullRequest {
  number: number;
  title: string;
  url: string;
  merged_at: string | null;
  additions: number;
  deletions: number;
  changed_files: number;
  test_files: string[];
  src_files: string[];
  languages: string[];
  linked_issues?: number[];
  labels?: string[];
  suitability: { score: number; reasons: string[]; flags: string[] };
  task_id: string | null;
}

export interface PrList {
  repo: string | null;
  since: string | null;
  limit: number;
  cached_s: number;
  prs: PullRequest[];
}

export interface TextFile {
  text: string | null;
  binary: boolean;
  size: number;
  truncated: boolean;
}

export interface TaskDetail {
  suite: { id: string; name: string };
  frozen: boolean;
  task: TaskRow;
  issue_md: TextFile | null;
  metadata: Record<string, unknown>;
  tests: (Partial<TextFile> & { path: string; omitted?: boolean })[];
  gold_patch: TextFile | null;
  author_notes: TextFile | null;
  scaffold: Record<string, unknown> | null;
  author: Record<string, unknown> | null;
  validate: Record<string, unknown> | null;
  jobs: {
    id: number;
    kind: string;
    status: string;
    created_at: string;
    finished_at: string | null;
    notes: string[];
    run_id: string;
  }[];
  log_run_id: string | null;
  options: SuiteDetail["options"];
}

export interface FreezeResult {
  id: string;
  path: string | null;
  version: string | null;
  lock_digest: string | null;
  n_tasks: number | null;
}

export const LOCK_CODES = ["builder_locked", "builder_not_configured", "wrong_password"];

/** The error means the builder is locked here: show the unlock screen again. */
export function isLocked(e: unknown): boolean {
  return e instanceof RunnerError && e.status === 403 && LOCK_CODES.includes(e.code ?? "");
}

export function suitePath(id: string): string {
  return `${SUITES_API}/${encodeURIComponent(id)}`;
}

export function taskPath(id: string, task: string): string {
  return `${suitePath(id)}/tasks/${encodeURIComponent(task)}`;
}

export const getAccess = () => runnerGet<AccessResponse>(`${SUITES_API}/access`);

/** The one call that carries the password: a JSON body to the loopback backend, nothing else. */
export const postUnlock = (password: string) =>
  runnerPost<AccessResponse>(`${SUITES_API}/unlock`, { password });

export const postLock = () => runnerPost<AccessResponse>(`${SUITES_API}/lock`);

/** A suite id from a display name, the way `suite init` wants it. */
export function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

/** "1 task", "2 tasks". */
export function plural(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`;
}

/**
 * owner/name from what people paste: owner/name itself, a github.com URL
 * (with or without a path after the repository, or .git), or a git@ remote.
 */
export function normalizeRepo(input: string): string {
  let s = input.trim();
  s = s.replace(/^git@github\.com:/i, "").replace(/^(https?:\/\/)?(www\.)?github\.com\//i, "");
  const parts = s.split(/[/?#]/).filter(Boolean);
  if (parts.length >= 2 && input.trim() !== s) s = `${parts[0]}/${parts[1]}`;
  return s.replace(/\.git$/i, "").replace(/\/+$/, "");
}

export const SUITE_ID_RE = /^[a-z0-9][a-z0-9-]{2,63}$/;
export const REPO_RE = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;

export const STAGE_LABEL: Record<Stage, string> = {
  missing: "missing",
  scaffolded: "scaffolded",
  authoring: "authoring",
  authored: "authored",
  author_failed: "authoring failed",
  validating: "validating",
  validated: "validated",
  validation_failed: "validation failed",
};
