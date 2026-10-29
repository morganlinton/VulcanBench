"use client";

import Link from "next/link";
import { useMemo } from "react";
import {
  fmtElapsed,
  fmtMoney,
  fmtTime,
  isActive,
  publicSuiteName,
  rewardValue,
  runnerGet,
  type FirstCheck,
  type RunView,
  type TrialView,
} from "@/lib/runner";
import { FirstCheckBadge, StatusBadge } from "./Badges";
import { LogTail, type LogChoice } from "./LogTail";
import { RunnerUnavailable } from "./RunnerUnavailable";
import { usePolling } from "./usePolling";

function Fact({ label, children, mono = false }: { label: string; children: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex justify-between gap-4 py-1 border-b border-white/5 last:border-0">
      <span className="text-zinc-400">{label}</span>
      <span className={`text-right text-zinc-200 ${mono ? "font-mono text-xs" : ""}`}>{children}</span>
    </div>
  );
}

function CheckDetail({ check }: { check: FirstCheck | null }) {
  if (!check) return null;
  const models = Object.entries(check.models);
  return (
    <div className="mt-2 text-xs text-zinc-400 space-y-1">
      <div className="flex items-center gap-2">
        <FirstCheckBadge check={check} />
        {check.evidence && <span className="font-mono text-zinc-500">from {check.evidence}</span>}
      </div>
      {models.length > 0 && (
        <div>
          models seen:{" "}
          {models.map(([m, n]) => (
            <span key={m} className={`font-mono mr-2 ${check.requested_model && m !== check.requested_model ? "text-rose-300" : "text-zinc-300"}`}>
              {m}{n > 0 ? ` (${n} turns)` : ""}
            </span>
          ))}
        </div>
      )}
      {check.rate_limit_status && <div>rate limit status: <span className="font-mono">{check.rate_limit_status}</span></div>}
      <ul className="list-disc pl-4">
        {check.reasons.map((r) => <li key={r}>{r}</li>)}
      </ul>
    </div>
  );
}

function RewardView({ trial }: { trial: TrialView }) {
  const v = rewardValue(trial.reward);
  if (trial.reward === null) {
    return <span className="text-zinc-500">{trial.status === "running" ? "pending" : "no reward"}</span>;
  }
  const flags = Object.entries(trial.reward).filter(([k]) => k !== "reward");
  return (
    <div>
      <span className={`font-mono text-base ${v === 1 ? "text-emerald-400" : "text-zinc-300"}`}>
        reward {String(trial.reward.reward ?? v ?? "n/a")}
      </span>
      {flags.length > 0 && (
        <div className="mt-1 flex flex-wrap gap-1">
          {flags.map(([k, val]) => (
            <span
              key={k}
              className={`rounded px-1.5 py-0.5 font-mono text-[10px] border ${val === 1 || val === 1.0 ? "border-emerald-500/30 text-emerald-300" : val === 0 ? "border-white/10 text-zinc-500" : "border-white/10 text-zinc-400"}`}
            >
              {k}={String(val)}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function TrialCard({ trial, index, total }: { trial: TrialView; index: number; total: number | null }) {
  return (
    <div className="border border-white/10 rounded-xl p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-3">
          <span className="text-zinc-400 text-sm">attempt {index + 1}{total ? ` of ${total}` : ""}</span>
          <span className="font-mono text-emerald-400">{trial.id}</span>
          <StatusBadge status={trial.status} />
        </div>
        <div className="text-xs text-zinc-500 font-mono">
          {fmtTime(trial.started_at)} · {fmtElapsed(trial.elapsed_s)}{trial.cost_usd !== null ? ` · ${fmtMoney(trial.cost_usd)}` : ""}
        </div>
      </div>
      <div className="mt-3 grid gap-4 md:grid-cols-2 text-sm">
        <div>
          <div className="text-xs uppercase tracking-wider text-zinc-500 mb-1">Reward</div>
          <RewardView trial={trial} />
          {trial.exception && <div className="mt-2 text-xs text-rose-300 font-mono break-all">{trial.exception}</div>}
        </div>
        <div>
          <div className="text-xs uppercase tracking-wider text-zinc-500 mb-1">Containers</div>
          {trial.containers.length === 0 ? (
            <span className="text-zinc-500 text-xs">{trial.status === "running" ? "none visible" : "none left behind"}</span>
          ) : (
            <ul className="text-xs font-mono space-y-0.5">
              {trial.containers.map((c) => (
                <li key={c.name}>
                  <span className={c.state === "running" ? "text-emerald-300" : "text-amber-300"}>{c.state}</span>{" "}
                  <span className="text-zinc-300">{c.name}</span> <span className="text-zinc-500">{c.status}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
      <div>
        <div className="mt-3 text-xs uppercase tracking-wider text-zinc-500">First attempt check</div>
        <CheckDetail check={trial.first_check} />
      </div>
    </div>
  );
}

export function RunDetail({ id }: { id: string }) {
  const { data: run, error, loading } = usePolling<RunView>(
    () => runnerGet<RunView>(`/api/runner/runs/${encodeURIComponent(id)}`),
    3000,
    id,
  );
  const live = run ? isActive(run.status) : true;

  const choices = useMemo<LogChoice[]>(() => {
    if (!run) return [];
    const out: LogChoice[] = [];
    const runningTrial = run.trials.find((t) => t.status === "running");
    const ordered = runningTrial ? [runningTrial, ...run.trials.filter((t) => t !== runningTrial)] : [...run.trials].reverse();
    for (const t of ordered) {
      for (const src of t.log_sources) {
        out.push({ key: `${t.id}:${src}`, label: `${t.id} / ${src}`, source: src, trial: t.id });
      }
    }
    for (const src of run.log_sources) {
      out.push({ key: `job:${src}`, label: `job / ${src.replace(/^\.\.\/\.\.\//, "")}`, source: src, trial: null });
    }
    // Harness runs carry their logs on the single trial and on the run; drop duplicates.
    const seen = new Set<string>();
    return out.filter((c) => (seen.has(c.key) ? false : (seen.add(c.key), true)));
  }, [run]);

  if (error && !run) {
    if (error.status === 404 && error.message.startsWith("run ")) {
      return (
        <div className="mt-6 text-sm text-zinc-400">
          Run not found under the configured roots. <Link href="/runs" className="text-emerald-400 hover:underline">Back to runs</Link>
        </div>
      );
    }
    return <RunnerUnavailable error={error} />;
  }
  if (!run) return <div className="mt-6 text-sm text-zinc-500">{loading ? "loading" : "no data"}</div>;

  const suite = publicSuiteName(run.suite_or_task);

  return (
    <div>
      <div className="flex flex-wrap items-center gap-3 mt-2">
        <h1 className="text-3xl font-semibold tracking-tight font-mono">{run.name}</h1>
        <StatusBadge status={run.status} />
        <FirstCheckBadge check={run.first_check} />
      </div>
      {run.notes.length > 0 && (
        <ul className="mt-2 text-xs text-amber-300 list-disc pl-4">
          {run.notes.map((n) => <li key={n}>{n}</li>)}
        </ul>
      )}

      <div className="mt-6 grid gap-4 md:grid-cols-2 text-sm">
        <div className="border border-white/10 rounded-xl p-5">
          <div className="text-zinc-400 mb-2">Run</div>
          <Fact label="kind">{run.kind}{run.owned ? "" : " (launched outside the GUI)"}</Fact>
          <Fact label={run.kind === "gate" ? "task" : "suite or task"}>
            <span>{suite ?? run.suite_or_task ?? "n/a"}</span>
            {suite && <span className="block font-mono text-xs text-zinc-500">{run.suite_or_task}</span>}
          </Fact>
          <Fact label="model" mono>{run.model ?? "n/a"}</Fact>
          <Fact label="harness">{run.harness ?? "n/a"}</Fact>
          <Fact label="effort">{run.effort ?? "n/a"}</Fact>
          <Fact label="attempts">{run.attempts_done}{run.attempts_total !== null ? ` of ${run.attempts_total}` : ""}{run.solves !== null ? `, ${run.solves} solved` : ""}</Fact>
        </div>
        <div className="border border-white/10 rounded-xl p-5">
          <div className="text-zinc-400 mb-2">Source and timing</div>
          <Fact label="bench tag" mono>{run.source_tag ?? <span className="text-amber-300">none</span>}</Fact>
          <Fact label="commit" mono>{run.source_commit ?? "n/a"}</Fact>
          {run.source_dir && <Fact label="checkout" mono>{run.source_dir}</Fact>}
          <Fact label="started">{fmtTime(run.started_at)}</Fact>
          <Fact label={live ? "elapsed" : "duration"}>{fmtElapsed(run.elapsed_s)}</Fact>
          {run.finished_at && <Fact label="finished">{fmtTime(run.finished_at)}</Fact>}
          {run.cost_usd !== null && <Fact label="cost (provider accounting)">{fmtMoney(run.cost_usd)}</Fact>}
          {run.pid !== null && <Fact label="pid" mono>{run.pid}</Fact>}
          {run.path && <Fact label="on disk" mono>{run.path}</Fact>}
        </div>
      </div>

      <h2 className="mt-8 text-sm uppercase tracking-[2px] text-zinc-500 mb-3">Attempts</h2>
      {run.trials.length === 0 ? (
        <div className="text-sm text-zinc-500 border border-white/10 rounded-xl p-4">No attempt has started yet.</div>
      ) : (
        <div className="space-y-3">
          {run.trials.map((t, i) => <TrialCard key={t.id} trial={t} index={i} total={run.attempts_total} />)}
        </div>
      )}

      <h2 className="mt-8 text-sm uppercase tracking-[2px] text-zinc-500 mb-3">Logs</h2>
      <LogTail runId={run.id} choices={choices} live={live} />
    </div>
  );
}
