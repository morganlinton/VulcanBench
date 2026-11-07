"use client";

import { useState } from "react";
import { runnerGet, type Readiness } from "@/lib/runner";
import { usePolling } from "./usePolling";

const DOT: Record<string, string> = {
  ok: "bg-emerald-400",
  warn: "bg-amber-400",
  fail: "bg-rose-400",
};

// Is this Mac ready? One read-only checklist (GET /api/runner/readiness):
// supervisor, a prepared bench tag, Docker, the gh login and the suite
// builder. Collapsed to one line when everything passes. Nothing here starts a
// job or calls a model; each harness's login is checked by the plan's doctor.
export function ReadinessCard() {
  const { data, loading, refresh } = usePolling<Readiness>(() => runnerGet<Readiness>("/api/runner/readiness"), 0, "readiness");
  const [open, setOpen] = useState<boolean | null>(null);
  if (!data) return null;
  const problems = data.checks.filter((c) => c.status !== "ok");
  const shown = open ?? problems.length > 0;
  return (
    <section className="mt-6 rounded-2xl border border-white/10 p-4 text-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <span className={`inline-block h-2 w-2 rounded-full ${DOT[data.status]}`} />
          <span className="text-zinc-200">
            {problems.length === 0
              ? "This Mac is ready for runs and suite building"
              : `This Mac needs ${problems.length} thing${problems.length > 1 ? "s" : ""} before everything works`}
          </span>
        </div>
        <div className="flex gap-3 text-xs">
          <button type="button" onClick={refresh} disabled={loading} className="text-zinc-400 hover:text-white disabled:opacity-50">
            {loading ? "checking" : "check again"}
          </button>
          <button type="button" onClick={() => setOpen(!shown)} className="text-emerald-400 hover:underline">
            {shown ? "hide" : "show"}
          </button>
        </div>
      </div>
      {shown && (
        <ul className="mt-3 space-y-2">
          {data.checks.map((c) => (
            <li key={c.id} className="flex gap-3">
              <span className={`mt-1.5 inline-block h-2 w-2 shrink-0 rounded-full ${DOT[c.status]}`} />
              <div>
                <div>
                  <span className="text-zinc-200">{c.label}</span>
                  <span className="ml-2 text-xs text-zinc-500">for {c.needed_for}</span>
                </div>
                <div className="text-xs text-zinc-400">{c.detail}</div>
                {c.status !== "ok" && c.fix && <div className="text-xs text-amber-200/90">{c.fix}</div>}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
