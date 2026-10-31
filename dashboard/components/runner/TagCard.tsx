"use client";

import { useState } from "react";
import { runnerGet, runnerPost, type TagPreflight, type TagResult } from "@/lib/runner";
import { button, field, label } from "./launchShared";
import { usePolling } from "./usePolling";

export function TagCard({ onCut }: { onCut: (tag: string | null) => void }) {
  const { data: pre, refresh } = usePolling<TagPreflight>(() => runnerGet<TagPreflight>("/api/runner/maintenance/tag"), 0, "tag-preflight");
  const [open, setOpen] = useState(false);
  const [labelText, setLabelText] = useState("");
  const [push, setPush] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<TagResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const today = new Date().toISOString().slice(0, 10);
  const cut = async () => {
    const what = `Cut bench/${today}-${labelText}${push ? " and push it to origin" : ""}? This tags the backend's own checkout; the script refuses unless it is on main, clean and equal to origin/main.`;
    if (!window.confirm(what)) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const r = await runnerPost<TagResult>("/api/runner/maintenance/tag", { label: labelText.trim(), push });
      setResult(r);
      onCut(r.tag);
      refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="rounded-2xl border border-white/10 p-5 text-sm">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <span className="text-zinc-200">Cut a bench tag</span>
          <span className="ml-2 text-xs text-zinc-500">scripts/cut_bench_tag.sh in {pre?.checkout ?? "the backend's checkout"}</span>
        </div>
        <button type="button" onClick={() => setOpen((v) => !v)} className="text-xs text-emerald-400 hover:underline">{open ? "hide" : "show"}</button>
      </div>
      {open && (
        <div className="mt-4 space-y-3">
          {pre && (
            <div className="text-xs">
              <span className="text-zinc-400">checkout: </span>
              <span className="font-mono text-zinc-200">{pre.branch ?? "?"} @ {pre.head ?? "?"}</span>
              <span className="text-zinc-500">, origin/main as last fetched {pre.origin_main ?? "?"}, {pre.clean ? "clean" : "not clean"}</span>
              {pre.problems.map((p) => <div key={p} className="text-amber-300">{p} (the script will refuse)</div>)}
            </div>
          )}
          <div className="grid gap-3 md:grid-cols-3 items-end">
            <div>
              <label className={label} htmlFor="taglabel">Label</label>
              <input id="taglabel" className={field} placeholder="terra-frontier-v4" value={labelText} onChange={(e) => setLabelText(e.target.value)} />
              <div className="mt-1 font-mono text-xs text-zinc-500">bench/{today}-{labelText || "<label>"}</div>
            </div>
            <label className="flex items-center gap-2 text-zinc-300 text-sm pb-2">
              <input type="checkbox" checked={push} onChange={(e) => setPush(e.target.checked)} />
              also push the tag to origin
            </label>
            <button type="button" onClick={cut} disabled={busy || !labelText.trim()} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
              {busy ? "running the script" : "Cut tag"}
            </button>
          </div>
          {result && (
            <pre className="rounded-lg bg-black/40 p-3 text-xs text-zinc-300 whitespace-pre-wrap break-all">{result.output}</pre>
          )}
          {error && <pre className="rounded-lg bg-black/40 p-3 text-xs text-rose-300 whitespace-pre-wrap break-all">{error}</pre>}
        </div>
      )}
    </div>
  );
}
