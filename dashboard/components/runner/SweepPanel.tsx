"use client";

import Link from "next/link";
import {
  fmtElapsed,
  fmtMoney,
  fmtTime,
  publicSuiteName,
  runHref,
  runnerGet,
  type SweepLevel,
  type SweepView,
  type SweepsData,
} from "@/lib/runner";
import { StatusBadge } from "./Badges";
import { StopButton } from "./QueuePanel";
import { usePolling } from "./usePolling";

function Bar({ frac, cls }: { frac: number; cls: string }) {
  return (
    <div className="h-1.5 w-full rounded bg-white/10 overflow-hidden">
      <div className={`h-full ${cls}`} style={{ width: `${Math.round(Math.min(1, Math.max(0, frac)) * 100)}%` }} />
    </div>
  );
}

function LevelRow({ lv, refresh }: { lv: SweepLevel; refresh: () => void }) {
  const { job, progress: p } = lv;
  const active = ["running", "stopping"].includes(job.status);
  const frac = p.total_runs ? p.runs_done / p.total_runs : 0;
  const cur = p.current;
  const resume = job.spec.resume_at;
  return (
    <tr className="hover:bg-white/5 align-top">
      <td className="p-3 font-mono text-sm text-zinc-200">
        <Link href={runHref(`job:${job.id}`)} className="text-emerald-400 hover:underline">{job.spec.effort ?? job.name}</Link>
      </td>
      <td className="p-3"><StatusBadge status={job.status} /></td>
      <td className="p-3 min-w-[180px]">
        <div className="flex justify-between text-xs">
          <span className="font-mono text-zinc-200">{p.runs_done} of {p.total_runs}</span>
          <span className="text-zinc-500">{p.passed} passed{p.pass_rate !== null ? ` (${Math.round(p.pass_rate * 100)}%)` : ""}</span>
        </div>
        <div className="mt-1"><Bar frac={frac} cls={p.complete ? "bg-zinc-300" : "bg-emerald-400"} /></div>
      </td>
      <td className="p-3 text-xs">
        {cur && active ? (
          <div>
            {cur.run_id_href ? (
              <Link href={runHref(cur.run_id_href)} className="font-mono text-emerald-400 hover:underline">{cur.task_id}</Link>
            ) : (
              <span className="font-mono text-zinc-200">{cur.task_id}</span>
            )}
            <div className="text-zinc-500">{fmtElapsed(cur.elapsed_s)} of the 3 h cap{p.stalled ? ", no trace activity for 15 min" : ""}</div>
            <div className="mt-1 w-32"><Bar frac={cur.budget_fraction ?? 0} cls={p.stalled ? "bg-amber-400" : "bg-sky-400"} /></div>
          </div>
        ) : job.status === "waiting" && resume ? (
          <span className="text-violet-300">quota window; continues {fmtTime(resume)}{job.spec.quota_continue ? ` (${job.spec.quota_continue.used ?? 0} of ${job.spec.quota_continue.max})` : ""}</span>
        ) : p.missing.length > 0 && !active && job.status !== "queued" ? (
          <span className="text-zinc-500" title={p.missing.join(", ")}>{p.missing.length} task{p.missing.length > 1 ? "s" : ""} without a result</span>
        ) : (
          <span className="text-zinc-600">{job.status === "queued" ? "waiting for the slot" : "none"}</span>
        )}
      </td>
      <td className="p-3 text-xs font-mono text-zinc-300">{p.cost_usd !== null ? fmtMoney(p.cost_usd) : "n/a"}</td>
      <td className="p-3 text-xs font-mono text-zinc-300">
        {active && p.eta_s !== null ? `~${fmtElapsed(p.eta_s)}` : active ? "after the first result" : "n/a"}
      </td>
      <td className="p-3"><StopButton job={job} onDone={refresh} /></td>
    </tr>
  );
}

function SweepCard({ sw, refresh }: { sw: SweepView; refresh: () => void }) {
  const missingLevels = sw.levels.filter((lv) => !lv.progress.complete && !["queued", "waiting", "running", "stopping", "preparing"].includes(lv.job.status));
  const resumeHref = missingLevels.length > 0 && sw.model && sw.harness && sw.suite && sw.tag
    ? `/launch?model=${encodeURIComponent(sw.model)}&harness=${encodeURIComponent(sw.harness)}&suite=${encodeURIComponent(sw.suite)}&efforts=${encodeURIComponent(missingLevels.map((lv) => lv.job.spec.effort ?? "").join(","))}&label=${encodeURIComponent(sw.name)}`
    : null;
  const t = sw.totals;
  return (
    <div className="rounded-xl border border-white/10">
      <div className="flex flex-wrap items-baseline justify-between gap-3 border-b border-white/10 px-4 py-3">
        <div>
          <span className="font-mono text-sm text-zinc-100">{sw.model ?? sw.name}</span>
          <span className="ml-2 text-xs text-zinc-400">{sw.harness} on {publicSuiteName(sw.suite) ?? sw.suite_name ?? sw.suite}</span>
          <span className="ml-2 font-mono text-xs text-zinc-500">{sw.tag}{sw.tag_commit ? ` @ ${sw.tag_commit}` : ""}</span>
        </div>
        <div className="text-xs text-zinc-400">
          <span className="font-mono text-zinc-200">{t.runs_done} of {t.total_runs}</span> runs, {t.passed} passed{t.cost_usd !== null ? `, ${fmtMoney(t.cost_usd)}` : ""}
          {resumeHref && (
            <Link href={resumeHref} className="ml-3 rounded-full border border-emerald-500/40 px-3 py-0.5 text-emerald-300 hover:bg-emerald-500/10">Run the missing tasks</Link>
          )}
        </div>
      </div>
      <table className="w-full text-sm">
        <thead className="bg-white/5 text-left text-xs text-zinc-400">
          <tr>
            {["Level", "Status", "Progress", "Current task", "Cost", "ETA", ""].map((h, i) => <th key={`${h}-${i}`} className="p-3 font-normal whitespace-nowrap">{h}</th>)}
          </tr>
        </thead>
        <tbody className="divide-y divide-white/10">
          {sw.levels.map((lv) => <LevelRow key={lv.job.id} lv={lv} refresh={refresh} />)}
        </tbody>
      </table>
    </div>
  );
}

export function SweepPanel() {
  const { data, refresh } = usePolling<SweepsData>(() => runnerGet<SweepsData>("/api/runner/sweeps"), 4000, "sweeps");
  if (!data || data.sweeps.length === 0) return null;
  const active = data.sweeps.filter((s) => s.totals.active);
  const recent = data.sweeps.filter((s) => !s.totals.active).slice(0, 3);
  return (
    <section className="mt-8">
      <h2 className="text-sm uppercase tracking-[2px] text-zinc-500 mb-3">Model sweeps <span className="text-zinc-600">({active.length} active)</span></h2>
      <div className="space-y-4">
        {[...active, ...recent].map((sw) => <SweepCard key={sw.sweep_id} sw={sw} refresh={refresh} />)}
      </div>
    </section>
  );
}
