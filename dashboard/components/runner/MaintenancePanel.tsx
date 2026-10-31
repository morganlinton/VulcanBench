"use client";

import Link from "next/link";
import { useState } from "react";
import { runHref, runnerGet, runnerPost, type OrphanData } from "@/lib/runner";
import { usePolling } from "./usePolling";

export function MaintenancePanel() {
  const { data, refresh } = usePolling<OrphanData>(() => runnerGet<OrphanData>("/api/runner/maintenance/orphans"), 15000, "orphans");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  if (!data || data.containers.length === 0) return null;
  const remove = async () => {
    const ids = data.containers.map((c) => c.id);
    if (!window.confirm(`Remove ${ids.length} orphaned sandbox container${ids.length > 1 ? "s" : ""}? Only the listed ones are touched; the list is re-checked first.`)) return;
    setBusy(true);
    setNote(null);
    try {
      const r = await runnerPost<{ removed: string[]; refused: string[]; networks: number; error: string | null }>("/api/runner/maintenance/orphans/remove", { ids });
      setNote(`removed ${r.removed.length}${r.networks ? `, ${r.networks} network(s)` : ""}${r.refused.length ? `; ${r.refused.length} no longer orphaned, left alone` : ""}${r.error ? `; ${r.error}` : ""}`);
      refresh();
    } catch (e) {
      setNote(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="mt-8">
      <div className="flex flex-wrap items-baseline justify-between gap-3 mb-3">
        <h2 className="text-sm uppercase tracking-[2px] text-zinc-500">
          Orphaned sandboxes <span className="text-zinc-600">({data.containers.length})</span>
        </h2>
        <button type="button" onClick={remove} disabled={busy} className="rounded-full border border-rose-500/40 px-4 py-1 text-xs text-rose-300 hover:bg-rose-500/10 disabled:opacity-50">
          {busy ? "removing" : "Remove the listed containers"}
        </button>
      </div>
      <div className="text-xs text-zinc-500 mb-2">
        Benchmark containers no active run owns. Nothing is removed by image or prefix while a run that could own it is alive.
        {data.live_runs.length > 0 && ` ${data.live_runs.length} run(s) are active; their containers are not listed.`}
      </div>
      {note && <div className="text-xs text-amber-300 mb-2">{note}</div>}
      <div className="overflow-x-auto border border-white/10 rounded-xl">
        <table className="w-full text-sm">
          <thead className="bg-white/5 text-left text-xs text-zinc-400">
            <tr>{["Container", "State", "Image", "Belonged to", "Why"].map((h) => <th key={h} className="p-3 font-normal whitespace-nowrap">{h}</th>)}</tr>
          </thead>
          <tbody className="divide-y divide-white/10">
            {data.containers.map((c) => (
              <tr key={c.id} className="hover:bg-white/5">
                <td className="p-3 font-mono text-xs text-zinc-200">{c.name}</td>
                <td className="p-3 text-xs text-zinc-400">{c.status}</td>
                <td className="p-3 font-mono text-xs text-zinc-400">{c.image}</td>
                <td className="p-3 font-mono text-xs">{c.run ? <Link href={runHref(c.run)} className="text-emerald-400 hover:underline">{c.run}</Link> : <span className="text-zinc-600">none on disk</span>}</td>
                <td className="p-3 text-xs text-zinc-400">{c.reason}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
