"use client";

import { useEffect, useState } from "react";
import type { WorktreeInfo } from "@/lib/runner";

export const field = "w-full rounded-md border border-white/10 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 focus:border-emerald-500/60 focus:outline-none";
export const label = "block text-xs uppercase tracking-wider text-zinc-500 mb-1";
export const button = "rounded-md px-4 py-2 text-sm font-medium disabled:opacity-40 disabled:cursor-not-allowed";

export function WorktreeCard({ wt, gate = false }: { wt: WorktreeInfo; gate?: boolean }) {
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
          {gate && !wt.script.status_file && <div className="text-amber-300">this tag&apos;s run_gate.sh predates the status file and the stand-alone usage check</div>}
        </div>
      ) : (
        <div className="mt-1 text-amber-300">
          {gate
            ? `not created yet: the plan will run scripts/run_worktree.sh ${wt.tag} first (make setup takes minutes)`
            : "not created yet: prepare it first (a queue step with no model calls), then build the plan"}
        </div>
      )}
      {wt.problems.filter((p) => wt.exists || !p.includes("not a git")).map((p) => (
        <div key={p} className="text-rose-300">{p}</div>
      ))}
    </div>
  );
}

export function Countdown({ until }: { until: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const left = Math.max(0, Math.floor((new Date(until).getTime() - now) / 1000));
  return <span className={left < 60 ? "text-rose-300" : "text-zinc-500"}>{left > 0 ? `plan valid for ${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}` : "plan expired"}</span>;
}

export function Warnings({ items }: { items: string[] }) {
  if (items.length === 0) return null;
  return (
    <ul className="list-disc pl-5 text-xs text-amber-300 space-y-0.5">
      {items.map((w) => <li key={w}>{w}</li>)}
    </ul>
  );
}

export function Row({ k, v, mono = true }: { k: string; v: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex justify-between gap-3"><dt className="text-zinc-400">{k}</dt><dd className={`text-right break-all ${mono ? "font-mono" : ""}`}>{v}</dd></div>
  );
}
