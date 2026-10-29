"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  commandLine,
  fmtDuration,
  fmtPercent,
  fmtTime,
  runnerGet,
  runnerPost,
  type GateRef,
  type JobSummary,
  type LaunchOptions,
  type LaunchPlan,
  type WorktreeInfo,
} from "@/lib/runner";
import { RunnerUnavailable } from "./RunnerUnavailable";
import { usePolling } from "./usePolling";

const field = "w-full rounded-md border border-white/10 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 focus:border-emerald-500/60 focus:outline-none";
const label = "block text-xs uppercase tracking-wider text-zinc-500 mb-1";
const button = "rounded-md px-4 py-2 text-sm font-medium disabled:opacity-40 disabled:cursor-not-allowed";

function RefPicker({ refs, value, onChange, credentials }: { refs: GateRef[]; value: string; onChange: (v: string) => void; credentials: LaunchOptions["credentials"] }) {
  return (
    <div className="grid gap-2 sm:grid-cols-3">
      {refs.map((r) => {
        const has = r.agent === "codex" ? credentials.codex_auth : credentials.claude_gate_env;
        const on = r.name === value;
        return (
          <button
            type="button"
            key={r.name}
            onClick={() => onChange(r.name)}
            className={`rounded-xl border p-3 text-left ${on ? "border-emerald-500/60 bg-emerald-500/10" : "border-white/10 hover:bg-white/5"}`}
          >
            <div className="font-mono text-sm text-zinc-100">{r.name}</div>
            <div className="text-xs text-zinc-400">{r.agent} {r.version}</div>
            <div className="font-mono text-xs text-zinc-300">{r.model}</div>
            <div className={`text-[11px] mt-1 ${has ? "text-emerald-300" : "text-rose-300"}`}>
              {has ? "credential file present" : "credential file missing"}
            </div>
          </button>
        );
      })}
    </div>
  );
}

function WorktreeCard({ wt }: { wt: WorktreeInfo }) {
  return (
    <div className="rounded-xl border border-white/10 p-3 text-xs">
      <div className="text-zinc-400">run worktree</div>
      <div className="font-mono text-zinc-200 break-all">{wt.path}</div>
      {wt.exists ? (
        <div className="mt-1 space-y-0.5">
          <div className={wt.at_tag ? "text-emerald-300" : "text-rose-300"}>
            {wt.at_tag ? `detached at ${wt.tag} (${wt.head})` : `HEAD ${wt.head ?? "?"} is not at ${wt.tag} (${wt.tag_commit ?? "?"})`}
          </div>
          {wt.dirty.length > 0 && <div className="text-rose-300">uncommitted: {wt.dirty.join(", ")}</div>}
          {!wt.script.status_file && <div className="text-amber-300">this tag&apos;s run_gate.sh predates the status file and the stand-alone usage check</div>}
        </div>
      ) : (
        <div className="mt-1 text-amber-300">
          not created yet: the plan will run scripts/run_worktree.sh {wt.tag} first (make setup takes minutes)
        </div>
      )}
      {wt.problems.filter((p) => wt.exists || !p.includes("not a git")).map((p) => (
        <div key={p} className="text-rose-300">{p}</div>
      ))}
    </div>
  );
}

function Countdown({ until }: { until: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const left = Math.max(0, Math.floor((new Date(until).getTime() - now) / 1000));
  return <span className={left < 60 ? "text-rose-300" : "text-zinc-500"}>{left > 0 ? `plan valid for ${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}` : "plan expired"}</span>;
}

function UsagePanel({ plan, onChecked }: { plan: LaunchPlan; onChecked: (p: LaunchPlan) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const spec = plan.spec;
  const check = plan.usage_check;
  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      onChecked(await runnerPost<LaunchPlan>(`/api/runner/plans/${plan.id}/usage`));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  if (spec.agent !== "claude-code") {
    return <div className="text-xs text-zinc-400">No usage pre-flight for {spec.agent}: the run uses the ChatGPT login in ~/.codex/auth.json.</div>;
  }
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={run} disabled={busy || !!plan.live_reason} className={`${button} border border-sky-500/40 text-sky-300 hover:bg-sky-500/10`}>
          {busy ? "checking (one short model turn)" : check ? "Check usage again" : "Check usage"}
        </button>
        <span className="text-xs text-zinc-500">
          runs <span className="font-mono">GATE_PREFLIGHT_ONLY=1 run_gate.sh {spec.ref}</span>: one one-turn call that reads the subscription windows
        </span>
      </div>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      {check && (
        <div className={`rounded-lg border p-3 text-xs ${check.would_start ? "border-emerald-500/30" : "border-rose-500/40"}`}>
          <div className="flex flex-wrap gap-4">
            <span>status <span className="font-mono text-zinc-200">{check.status}</span></span>
            {check.max_utilization !== undefined && (
              <span>highest window <span className={`font-mono ${check.would_start ? "text-emerald-300" : "text-rose-300"}`}>{fmtPercent(check.max_utilization)}</span> of {fmtPercent(check.max_allowed)} allowed</span>
            )}
            <span className="text-zinc-500">checked {fmtTime(plan.usage_checked_at)}</span>
          </div>
          {check.windows && (
            <div className="mt-1 font-mono text-zinc-400">
              {Object.entries(check.windows).map(([k, v]) => `${k} ${fmtPercent(v)}`).join("  ")}
            </div>
          )}
          {check.note && <div className="mt-1 text-zinc-400">{check.note}</div>}
          {!check.would_start && <div className="mt-1 text-rose-300">run_gate.sh would refuse to start; confirm is disabled.</div>}
        </div>
      )}
      {!check && <div className="text-xs text-amber-300">Confirm is disabled until the usage check has run in the last 10 minutes.</div>}
    </div>
  );
}

function PlanCard({ plan, onPlan, onQueued }: { plan: LaunchPlan; onPlan: (p: LaunchPlan) => void; onQueued: (j: JobSummary) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const spec = plan.spec;
  const est = spec.estimate;
  const canConfirm = !plan.live_reason && plan.usage_check_ok && !busy;
  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      onQueued(await runnerPost<JobSummary>(`/api/runner/plans/${plan.id}/confirm`, { hash: plan.hash }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      try {
        onPlan(await runnerGet<LaunchPlan>(`/api/runner/plans/${plan.id}`));
      } catch {
        // keep what we have
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="mt-6 rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.03] p-5 space-y-5">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-semibold">Plan: <span className="font-mono text-emerald-300">{spec.job_name}</span></h2>
        <div className="text-xs"><Countdown until={plan.expires_at} /> <span className="text-zinc-600 ml-2 font-mono">hash {plan.hash}</span></div>
      </div>

      {spec.warnings.length > 0 && (
        <ul className="list-disc pl-5 text-xs text-amber-300 space-y-0.5">
          {spec.warnings.map((w) => <li key={w}>{w}</li>)}
        </ul>
      )}

      <div>
        <div className={label}>Exactly what runs</div>
        {spec.prep_argv && (
          <pre className="rounded-lg bg-black/40 p-3 text-xs text-zinc-300 overflow-x-auto whitespace-pre-wrap break-all">
            <span className="text-zinc-500"># 1. prepare the run worktree (no model calls)\n</span>
            {commandLine(spec.prep_env, spec.prep_argv)}
          </pre>
        )}
        <pre className="mt-2 rounded-lg bg-black/40 p-3 text-xs text-zinc-200 overflow-x-auto whitespace-pre-wrap break-all">
          <span className="text-zinc-500"># {spec.prep_argv ? "2. " : ""}cd {spec.cwd}\n</span>
          {commandLine(spec.env, spec.argv)}
        </pre>
        <div className="mt-1 text-xs text-zinc-500">
          child environment: {Object.keys(spec.env).join(", ")} plus {spec.env_passthrough.join(", ")} passed through; nothing else (no VULCANBENCH_*, no keys or tokens).
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 text-sm">
        <div className="rounded-xl border border-white/10 p-4">
          <div className={label}>Estimate</div>
          <dl className="space-y-1 text-xs">
            <div className="flex justify-between"><dt className="text-zinc-400">attempts</dt><dd className="font-mono">{est.attempts}, {est.concurrency} at a time</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">task timeout</dt><dd className="font-mono">{fmtDuration(est.task_timeout_s)} per attempt</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">longest wall time</dt><dd className="font-mono">{fmtDuration(est.max_wall_s)}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">billing</dt><dd className="text-right">{est.billing}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">early exit</dt><dd className="text-right">{est.early_exit}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">retries</dt><dd className="text-right">{est.infra_retries}</dd></div>
          </dl>
        </div>
        <div className="rounded-xl border border-white/10 p-4">
          <div className={label}>Source</div>
          <dl className="space-y-1 text-xs">
            <div className="flex justify-between"><dt className="text-zinc-400">bench tag</dt><dd className="font-mono">{spec.tag}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">commit</dt><dd className="font-mono">{spec.tag_commit ?? "n/a"}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">model</dt><dd className="font-mono">{spec.model}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">harness</dt><dd className="font-mono">{spec.agent} {spec.harness_version}</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">effort</dt><dd className="font-mono">{spec.effort} (fixed by run_gate.sh)</dd></div>
            <div className="flex justify-between"><dt className="text-zinc-400">output</dt><dd className="font-mono text-right break-all">{spec.job_dir}</dd></div>
          </dl>
        </div>
      </div>

      <div>
        <div className={label}>Usage</div>
        <UsagePanel plan={plan} onChecked={onPlan} />
      </div>

      <div className="flex flex-wrap items-center gap-4 border-t border-white/10 pt-4">
        <button type="button" onClick={confirm} disabled={!canConfirm} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
          {busy ? "queueing" : "Confirm and queue this run"}
        </button>
        <span className="text-xs text-zinc-500">
          Queues the job; the supervisor starts it when nothing else is running. Nothing retries on failure.
        </span>
        {plan.live_reason && <span className="text-xs text-rose-300">{plan.live_reason}</span>}
        {error && <span className="text-xs text-rose-300">{error}</span>}
      </div>
    </div>
  );
}

export function LaunchForm() {
  const router = useRouter();
  const [tagChoice, setTagChoice] = useState<string | null>(null);
  const { data: opts, error } = usePolling<LaunchOptions>(
    () => runnerGet<LaunchOptions>(`/api/runner/options${tagChoice ? `?tag=${encodeURIComponent(tagChoice)}` : ""}`),
    0,
    tagChoice ?? "",
  );
  const [ref, setRef] = useState("");
  const [taskChoice, setTaskChoice] = useState<string | null>(null);
  const [attempts, setAttempts] = useState(3);
  const [concurrency, setConcurrency] = useState<"" | "1" | "2">("");
  const [suffix, setSuffix] = useState("");
  const [plan, setPlan] = useState<LaunchPlan | null>(null);
  const [building, setBuilding] = useState(false);
  const [buildError, setBuildError] = useState<string | null>(null);
  const [queued, setQueued] = useState<JobSummary | null>(null);

  if (error && !opts) return <RunnerUnavailable error={error} />;
  if (!opts) return <div className="mt-6 text-sm text-zinc-500">loading options</div>;

  const tag = opts.tag ?? "";
  const refName = opts.refs.some((r) => r.name === ref) ? ref : (opts.refs[0]?.name ?? "");
  const task = taskChoice && opts.tasks.some((t) => t.name === taskChoice) ? taskChoice : (opts.tasks[0]?.name ?? "");
  const taskInfo = opts.tasks.find((t) => t.name === task);
  const formReady = !!tag && !!refName && !!task && !(opts.worktree?.exists && opts.worktree.problems.length > 0);

  const build = async () => {
    setBuilding(true);
    setBuildError(null);
    setPlan(null);
    try {
      const p = await runnerPost<LaunchPlan>("/api/runner/plans", {
        kind: "gate",
        ref: refName,
        task,
        tag,
        attempts,
        concurrency: concurrency ? Number(concurrency) : null,
        suffix,
      });
      setPlan(p);
    } catch (e) {
      setBuildError(e instanceof Error ? e.message : String(e));
    } finally {
      setBuilding(false);
    }
  };

  if (queued) {
    return (
      <div className="mt-6 rounded-2xl border border-emerald-500/40 p-5 text-sm">
        <div className="text-emerald-300 font-medium">Queued as job {queued.id}: <span className="font-mono">{queued.name}</span></div>
        <div className="mt-1 text-zinc-400">The supervisor starts it when nothing else is running. Watch it on the Runs page.</div>
        <div className="mt-3 flex gap-3">
          <button type="button" onClick={() => router.push("/runs")} className={`${button} bg-emerald-500 text-black`}>Go to runs</button>
          <button type="button" onClick={() => { setQueued(null); setPlan(null); }} className={`${button} border border-white/15 text-zinc-200`}>Plan another</button>
        </div>
      </div>
    );
  }

  return (
    <div className="mt-6 space-y-6">
      <div className="rounded-2xl border border-white/10 p-5 space-y-5">
        <div>
          <div className={label}>Kind</div>
          <div className="flex gap-2 text-sm">
            <span className="rounded-full border border-emerald-500/60 bg-emerald-500/10 px-3 py-1">Frontier v5 gate run</span>
            <span className="rounded-full border border-white/10 px-3 py-1 text-zinc-500" title="Milestone 3">suite or task run (coming next)</span>
          </div>
        </div>

        <div>
          <div className={label}>Reference model</div>
          <RefPicker refs={opts.refs} value={refName} onChange={(v) => { setRef(v); setPlan(null); }} credentials={opts.credentials} />
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <label className={label} htmlFor="tag">Bench tag</label>
            <select id="tag" className={field} value={tag} onChange={(e) => { setTagChoice(e.target.value); setTaskChoice(null); setPlan(null); }}>
              {opts.tags.map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
            <div className="mt-2">{opts.worktree && <WorktreeCard wt={opts.worktree} />}</div>
          </div>
          <div>
            <label className={label} htmlFor="task">Task at this tag</label>
            <select id="task" className={field} value={task} onChange={(e) => { setTaskChoice(e.target.value); setPlan(null); }}>
              {opts.tasks.map((t) => <option key={t.name} value={t.name}>{t.name}{t.language ? ` (${t.language})` : ""}</option>)}
            </select>
            {opts.tasks.length === 0 && <div className="mt-1 text-xs text-amber-300">No tasks/frontier-v5 at this tag.</div>}
            <div className="mt-3 grid grid-cols-3 gap-3">
              <div>
                <label className={label} htmlFor="attempts">Attempts</label>
                <select id="attempts" className={field} value={attempts} onChange={(e) => { setAttempts(Number(e.target.value)); setPlan(null); }}>
                  {opts.attempts.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              </div>
              <div>
                <label className={label} htmlFor="conc">Concurrency</label>
                <select id="conc" className={field} value={concurrency} onChange={(e) => { setConcurrency(e.target.value as "" | "1" | "2"); setPlan(null); }}>
                  <option value="">task default ({taskInfo?.default_concurrency ?? "?"})</option>
                  {opts.concurrency.map((n) => <option key={n} value={String(n)}>{n}</option>)}
                </select>
              </div>
              <div>
                <label className={label} htmlFor="suffix">Job suffix</label>
                <input id="suffix" className={field} placeholder="-topup" value={suffix} onChange={(e) => { setSuffix(e.target.value); setPlan(null); }} />
              </div>
            </div>
            <div className="mt-2 text-xs text-zinc-500">
              effort: <span className="font-mono">{opts.gate_effort}</span>, fixed by run_gate.sh. Blocked levels are never offered.
            </div>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-4 border-t border-white/10 pt-4">
          <button type="button" onClick={build} disabled={!formReady || building} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
            {building ? "building plan" : "Build plan"}
          </button>
          <span className="text-xs text-zinc-500">Builds and validates the exact command. Nothing starts yet.</span>
          {buildError && <span className="text-xs text-rose-300">{buildError}</span>}
        </div>
      </div>

      {plan && <PlanCard plan={plan} onPlan={setPlan} onQueued={setQueued} />}

      <p className="text-xs text-zinc-600">
        Every launch runs from a worktree under <span className="font-mono">{opts.run_worktrees}</span> pinned to its bench tag, never from the working checkout and never with --allow-dirty. Output goes to <span className="font-mono">{opts.gate_out}</span>. See <Link href="/runs" className="text-emerald-400 hover:underline">Runs</Link> for the queue.
      </p>
    </div>
  );
}
