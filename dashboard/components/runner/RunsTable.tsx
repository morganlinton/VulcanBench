"use client";

import Link from "next/link";
import { useMemo } from "react";
import {
  fmtElapsed,
  fmtTime,
  isActive,
  publicSuiteName,
  runHref,
  runnerGet,
  type RunRow,
} from "@/lib/runner";
import { FirstCheckBadge, RewardDots, StatusBadge } from "./Badges";
import { usePolling } from "./usePolling";
import { RunnerUnavailable } from "./RunnerUnavailable";
import { QueuePanel } from "./QueuePanel";
import { SweepPanel } from "./SweepPanel";

function SourceCell({ run }: { run: RunRow }) {
  if (!run.source_tag && !run.source_commit) return <span className="text-zinc-600">unknown</span>;
  return (
    <span className="font-mono text-xs">
      {run.source_tag ? (
        <span className="text-zinc-300">{run.source_tag.replace(/^bench\//, "bench/")}</span>
      ) : (
        <span className="text-amber-300" title="checkout is not at a bench tag">no bench tag</span>
      )}
      {run.source_commit && <span className="text-zinc-500"> @ {run.source_commit}</span>}
    </span>
  );
}

function Row({ run }: { run: RunRow }) {
  const suite = publicSuiteName(run.suite_or_task);
  return (
    <tr className="hover:bg-white/5">
      <td className="p-3 align-top"><StatusBadge status={run.status} /></td>
      <td className="p-3 align-top">
        <Link href={runHref(run.id)} className="font-mono text-emerald-400 hover:underline">{run.name}</Link>
        <div className="text-xs text-zinc-500">{run.kind}{run.owned ? ", from the GUI" : ", external"}</div>
      </td>
      <td className="p-3 align-top">
        <div className="text-zinc-200">{suite ?? run.suite_or_task ?? "n/a"}</div>
        {suite && <div className="font-mono text-xs text-zinc-500">{run.suite_or_task}</div>}
      </td>
      <td className="p-3 align-top font-mono text-xs text-zinc-300">{run.model ?? "n/a"}</td>
      <td className="p-3 align-top text-zinc-300">{run.harness ?? "n/a"}</td>
      <td className="p-3 align-top text-zinc-300">{run.effort ?? "n/a"}</td>
      <td className="p-3 align-top"><SourceCell run={run} /></td>
      <td className="p-3 align-top font-mono text-zinc-300">
        {run.attempts_done}{run.attempts_total !== null ? ` of ${run.attempts_total}` : ""}
        {run.solves !== null && run.solves > 0 && (
          <span className="ml-2 text-emerald-400">{run.solves} solved</span>
        )}
      </td>
      <td className="p-3 align-top"><RewardDots trials={run.trials} /></td>
      <td className="p-3 align-top font-mono text-zinc-300" title={`started ${fmtTime(run.started_at)}`}>
        {fmtElapsed(run.elapsed_s)}
      </td>
      <td className="p-3 align-top"><FirstCheckBadge check={run.first_check} compact /></td>
    </tr>
  );
}

function Section({ title, runs, empty }: { title: string; runs: RunRow[]; empty: string }) {
  return (
    <section className="mt-8">
      <h2 className="text-sm uppercase tracking-[2px] text-zinc-500 mb-3">{title} <span className="text-zinc-600">({runs.length})</span></h2>
      {runs.length === 0 ? (
        <div className="text-sm text-zinc-500 border border-white/10 rounded-xl p-4">{empty}</div>
      ) : (
        <div className="overflow-x-auto border border-white/10 rounded-xl">
          <table className="w-full text-sm">
            <thead className="bg-white/5 text-left">
              <tr>
                {["Status", "Run", "Suite or task", "Model", "Harness", "Effort", "Source", "Attempts", "Rewards", "Elapsed", "First attempt"].map((h) => (
                  <th key={h} className="p-3 font-normal text-zinc-400 whitespace-nowrap">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-white/10">
              {runs.map((r) => <Row key={r.id} run={r} />)}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

export function RunsTable() {
  const { data, error, loading, updatedAt } = usePolling<RunRow[]>(
    () => runnerGet<RunRow[]>("/api/runner/runs"),
    4000,
    "runs",
  );
  const [active, done] = useMemo(() => {
    const rows = data ?? [];
    return [rows.filter((r) => isActive(r.status)), rows.filter((r) => !isActive(r.status))];
  }, [data]);

  if (error && !data) return <RunnerUnavailable error={error} />;

  return (
    <div>
      <div className="flex items-center gap-3 text-xs text-zinc-500">
        {loading && !data ? "loading" : updatedAt ? `refreshed ${updatedAt.toLocaleTimeString()}` : ""}
        {error && data && <span className="text-amber-300">last refresh failed: {error.message}</span>}
        <span className="text-zinc-600">polls every 4 s</span>
      </div>
      <SweepPanel />
      <QueuePanel />
      <Section title="Active" runs={active} empty="Nothing is running. Runs launched by hand appear here as external once their process is visible." />
      <Section title="Finished" runs={done} empty="No finished runs under the configured roots." />
    </div>
  );
}
