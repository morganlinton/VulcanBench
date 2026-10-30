"use client";

import Link from "next/link";
import { useState } from "react";
import {
  fmtTime,
  runHref,
  runnerDelete,
  runnerGet,
  runnerPost,
  type JobSummary,
  type QueueData,
} from "@/lib/runner";
import { StatusBadge } from "./Badges";
import { usePolling } from "./usePolling";

const SHOW_FINISHED = 5;

function SupervisorLine({ q }: { q: QueueData }) {
  const s = q.supervisor;
  let text: string;
  let cls = "text-zinc-400";
  if (s.status === "active") {
    text = s.hold_reason ? s.hold_reason : `supervisor active, last tick ${fmtTime(s.last_tick ?? null)}`;
    if (s.hold_reason) cls = "text-amber-300";
  } else if (s.status === "inactive") {
    text = `supervisor inactive: ${s.detail ?? "another backend holds the lock"}`;
    cls = "text-amber-300";
  } else {
    text = "supervisor not running in this backend; queued jobs wait";
    cls = "text-amber-300";
  }
  return (
    <div className={`text-xs ${cls}`}>
      {text}
      {s.last_error && <span className="ml-2 text-rose-300">last tick failed: {s.last_error}</span>}
      {q.outside.length > 0 && (
        <span className="ml-2 text-zinc-500">
          {q.outside.length} run{q.outside.length > 1 ? "s" : ""} launched outside the GUI
        </span>
      )}
    </div>
  );
}

export function StopButton({
  job,
  onDone,
  label = "Stop",
}: {
  job: { id: number; status: string; name: string };
  onDone?: () => void;
  label?: string;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const queued = job.status === "queued";
  const act = async () => {
    const what = queued ? `Remove ${job.name} from the queue?` : `Stop ${job.name}? Harbor cancels the running attempt and the sandboxes are removed.`;
    if (!window.confirm(what)) return;
    setBusy(true);
    setError(null);
    try {
      if (queued) await runnerDelete(`/api/runner/jobs/${job.id}`);
      else await runnerPost(`/api/runner/jobs/${job.id}/stop`, { reason: "stopped from the GUI" });
      onDone?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  if (!["queued", "preparing", "running"].includes(job.status)) return null;
  return (
    <span className="inline-flex items-center gap-2">
      <button
        type="button"
        onClick={act}
        disabled={busy}
        className="rounded-md border border-rose-500/40 px-2.5 py-1 text-xs text-rose-300 hover:bg-rose-500/10 disabled:opacity-50"
      >
        {queued ? "Remove" : label}
      </button>
      {error && <span className="text-xs text-rose-300">{error}</span>}
    </span>
  );
}

function JobRow({ job, refresh }: { job: JobSummary; refresh: () => void }) {
  const spec = job.spec;
  return (
    <tr className="hover:bg-white/5">
      <td className="p-3 align-top"><StatusBadge status={job.status} /></td>
      <td className="p-3 align-top">
        <Link href={runHref(`job:${job.id}`)} className="font-mono text-emerald-400 hover:underline">{job.name}</Link>
        <div className="text-xs text-zinc-500">
          job {job.id}{job.phase === "prepare" ? ", preparing the worktree" : ""}
          {job.stop_requested_at && job.status !== "stopped" ? ", stop requested" : ""}
        </div>
      </td>
      <td className="p-3 align-top font-mono text-xs text-zinc-300">{spec.tag ?? "n/a"}</td>
      <td className="p-3 align-top font-mono text-xs text-zinc-400 max-w-md truncate" title={spec.argv?.join(" ")}>
        {spec.argv ? spec.argv.slice(1).map((a) => a.split("/").pop()).join(" ") : "n/a"}
      </td>
      <td className="p-3 align-top text-xs text-zinc-400">{fmtTime(job.started_at ?? job.created_at)}</td>
      <td className="p-3 align-top"><StopButton job={job} onDone={refresh} /></td>
    </tr>
  );
}

export function QueuePanel() {
  const { data, error, refresh } = usePolling<QueueData>(
    () => runnerGet<QueueData>("/api/runner/queue"),
    3000,
    "queue",
  );
  if (error && !data) return null; // the runs table shows the unavailable notice
  if (!data) return null;
  const live = data.jobs.filter((j) => ["queued", "preparing", "running", "stopping"].includes(j.status));
  const recent = data.jobs.filter((j) => !live.includes(j)).slice(0, SHOW_FINISHED);
  return (
    <section className="mt-8">
      <div className="flex flex-wrap items-baseline justify-between gap-3 mb-3">
        <h2 className="text-sm uppercase tracking-[2px] text-zinc-500">
          Queue <span className="text-zinc-600">({live.length})</span>
        </h2>
        <Link href="/launch" className="rounded-full border border-emerald-500/40 px-4 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10">
          Launch a run
        </Link>
      </div>
      <SupervisorLine q={data} />
      {live.length === 0 && recent.length === 0 ? (
        <div className="mt-3 text-sm text-zinc-500 border border-white/10 rounded-xl p-4">
          Nothing queued. Runs start only from the Launch page, one at a time, after a confirm click.
        </div>
      ) : (
        <div className="mt-3 overflow-x-auto border border-white/10 rounded-xl">
          <table className="w-full text-sm">
            <thead className="bg-white/5 text-left">
              <tr>
                {["Status", "Job", "Tag", "Command", "Started", ""].map((h, i) => (
                  <th key={`${h}-${i}`} className="p-3 font-normal text-zinc-400 whitespace-nowrap">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-white/10">
              {live.map((j) => <JobRow key={j.id} job={j} refresh={refresh} />)}
              {recent.map((j) => <JobRow key={j.id} job={j} refresh={refresh} />)}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
