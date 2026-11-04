"use client";

import Link from "next/link";
import { useState } from "react";
import { Countdown, Row, Warnings, button, field, label } from "@/components/runner/launchShared";
import {
  commandLine,
  fmtDuration,
  runnerPost,
  type DoctorResult,
  type JobSummary,
  type LaunchPlan,
} from "@/lib/runner";
import type { SuiteDetail } from "@/lib/suites";
import { useGate } from "./BuilderGate";

export interface BuildJob {
  task: string;
  argv: string[];
  cwd: string;
  env: Record<string, string>;
}

export interface BuildPlanSpec {
  kind: "author" | "validate";
  suite: string;
  suite_name: string;
  tasks: string[];
  harness: string | null;
  model: string | null;
  billing: string | null;
  effort: string | null;
  tag: string;
  tag_commit: string | null;
  worktree: string;
  path_prepend: string;
  jobs: BuildJob[];
  doctor: DoctorResult;
  estimate: {
    sessions: number;
    per_session_cap_s?: number;
    max_turns?: number;
    max_wall_s: number | null;
    billing: string;
    cost: string;
    quota: string;
    infra_retries: string;
  };
  warnings: string[];
}

type Kind = "author" | "validate";

// Build an author or validate plan for some tasks, show exactly what would
// run, and queue it only on the confirm click with the plan's hash. Nothing
// here starts a session on its own, and nothing retries.
export function BuildActions({
  suite,
  tasks,
  options,
  kind,
  onDone,
}: {
  suite: string;
  tasks: string[];
  options: SuiteDetail["options"];
  kind: Kind;
  onDone: () => void;
}) {
  const { handle } = useGate();
  const [harness, setHarness] = useState(options.harnesses.includes("codex") ? "codex" : options.harnesses[0] ?? "");
  const [model, setModel] = useState("");
  const [billing, setBilling] = useState(options.billing[0] ?? "auto");
  const [effort, setEffort] = useState(options.efforts.includes("medium") ? "medium" : "");
  const [plan, setPlan] = useState<LaunchPlan<BuildPlanSpec> | null>(null);
  const [building, setBuilding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [queued, setQueued] = useState<JobSummary[] | null>(null);
  const ready = tasks.length > 0 && (kind === "validate" || (harness && model.trim()));

  const build = async () => {
    setBuilding(true);
    setError(null);
    setPlan(null);
    try {
      const body =
        kind === "author"
          ? { kind, suite, tasks, harness, model: model.trim(), billing, effort: effort || null }
          : { kind, suite, tasks };
      setPlan(await runnerPost<LaunchPlan<BuildPlanSpec>>("/api/runner/plans", body));
    } catch (e) {
      setError(handle(e));
    } finally {
      setBuilding(false);
    }
  };

  if (queued) {
    return (
      <div className="rounded-xl border border-emerald-500/40 p-4 text-sm space-y-2">
        <div className="text-emerald-300">
          Queued {queued.length} {kind === "author" ? "authoring session" : "validation"}
          {queued.length > 1 ? "s" : ""}. The supervisor runs them one at a time, after anything already queued.
        </div>
        <div className="flex flex-wrap gap-3 text-xs">
          {queued.map((j) => (
            <Link key={j.id} href={`/runs/${encodeURIComponent(`job:${j.id}`)}`} className="text-emerald-300 hover:underline">
              job {j.id} {j.spec.task ?? ""}
            </Link>
          ))}
        </div>
        <button type="button" onClick={() => { setQueued(null); setPlan(null); onDone(); }} className="text-xs text-zinc-400 hover:text-white">
          done
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {kind === "author" && (
        <div className="grid gap-3 sm:grid-cols-4">
          <div>
            <label className={label} htmlFor="author-harness">Harness</label>
            <select id="author-harness" className={field} value={harness} onChange={(e) => setHarness(e.target.value)}>
              {options.harnesses.map((h) => <option key={h} value={h}>{h}</option>)}
            </select>
          </div>
          <div>
            <label className={label} htmlFor="author-model">Model</label>
            <input id="author-model" className={`${field} font-mono`} value={model} onChange={(e) => setModel(e.target.value)} placeholder="gpt-6.1-terra" />
          </div>
          <div>
            <label className={label} htmlFor="author-billing">Billing</label>
            <select id="author-billing" className={field} value={billing} onChange={(e) => setBilling(e.target.value)}>
              {options.billing.map((b) => <option key={b} value={b}>{b}</option>)}
            </select>
          </div>
          <div>
            <label className={label} htmlFor="author-effort">Effort</label>
            <select id="author-effort" className={field} value={effort} onChange={(e) => setEffort(e.target.value)}>
              <option value="">harness default</option>
              {options.efforts.map((x) => <option key={x} value={x}>{x}</option>)}
            </select>
          </div>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={build} disabled={!ready || building} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
          {building ? "Building plan" : "Build plan"}
        </button>
        <span className="text-xs text-zinc-500">
          {tasks.length} task{tasks.length === 1 ? "" : "s"}: {tasks.join(", ") || "select some in the table"}
        </span>
      </div>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      {plan && <BuildPlanCard plan={plan} onQueued={setQueued} />}
    </div>
  );
}

function BuildPlanCard({ plan, onQueued }: { plan: LaunchPlan<BuildPlanSpec>; onQueued: (jobs: JobSummary[]) => void }) {
  const { handle } = useGate();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const spec = plan.spec;
  const est = spec.estimate;
  const author = spec.kind === "author";
  const doctorBlocks = spec.doctor.status === "not_ready";
  const canConfirm = !plan.live_reason && !doctorBlocks && !busy;
  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      const first = await runnerPost<JobSummary>(`/api/runner/plans/${plan.id}/confirm`, { hash: plan.hash });
      onQueued(first.jobs ?? [first]);
    } catch (e) {
      setError(handle(e));
    } finally {
      setBusy(false);
    }
  };
  const shown = showAll ? spec.jobs : spec.jobs.slice(0, 1);
  const n = spec.jobs.length;
  return (
    <div className="rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.03] p-5 space-y-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="text-base font-semibold">{author ? "Authoring plan" : "Validation plan"}</h3>
        <div className="text-xs"><Countdown until={plan.expires_at} /> <span className="ml-2 font-mono text-zinc-600">hash {plan.hash}</span></div>
      </div>
      <Warnings items={spec.warnings} />
      {author && (
        <div className={`text-xs ${spec.doctor.status === "ready" ? "text-emerald-300" : "text-rose-300"}`}>
          harness doctor: {spec.doctor.status === "ready"
            ? `${spec.harness} ${spec.doctor.version ?? ""} ready, ${spec.doctor.auth_mode ?? ""}. No model call was made.`
            : `${spec.doctor.status}: ${spec.doctor.detail ?? spec.doctor.note ?? ""}${doctorBlocks ? " (confirm is disabled)" : ""}`}
        </div>
      )}
      <div>
        <div className={label}>Exactly what runs ({n} queued job{n > 1 ? "s" : ""}, one per task, one at a time)</div>
        {shown.map((j) => (
          <pre key={j.task} className="mt-2 overflow-x-auto whitespace-pre-wrap break-all rounded-lg bg-black/40 p-3 text-xs text-zinc-200">
            <span className="text-zinc-500">{`# ${j.task}: cd ${j.cwd}\n`}</span>
            {commandLine(j.env, j.argv)}
          </pre>
        ))}
        {n > 1 && (
          <button type="button" onClick={() => setShowAll((v) => !v)} className="mt-1 text-xs text-emerald-400 hover:underline">
            {showAll ? "show the first task only" : `show all ${n} (only --task differs)`}
          </button>
        )}
      </div>
      <dl className="max-w-2xl space-y-1 text-xs">
        {author && <Row k="sessions" v={`${est.sessions}, one per task`} />}
        {author && est.per_session_cap_s && <Row k="per session" v={`at most ${fmtDuration(est.per_session_cap_s)}, ${est.max_turns} turns`} />}
        {est.max_wall_s !== null && <Row k="longest wall time" v={fmtDuration(est.max_wall_s)} />}
        {author && <Row k="model" v={`${spec.harness}:${spec.model}${spec.effort ? ` at ${spec.effort}` : ""}`} />}
        <Row k="billing" v={est.billing} mono={false} />
        <Row k="cost" v={est.cost} mono={false} />
        <Row k="quota" v={est.quota} mono={false} />
        <Row k="retries" v={est.infra_retries} mono={false} />
        <Row k="bench tag" v={`${spec.tag} @ ${spec.tag_commit ?? "?"}`} />
      </dl>
      <div className="flex flex-wrap items-center gap-4 border-t border-white/10 pt-4">
        <button type="button" onClick={confirm} disabled={!canConfirm} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
          {busy ? "Queueing" : author ? `Confirm and queue ${n} authoring session${n > 1 ? "s" : ""}` : `Confirm and queue ${n} validation${n > 1 ? "s" : ""}`}
        </button>
        <span className="text-xs text-zinc-500">
          {author
            ? "Each session spends model quota once. Nothing retries a failed or refused session; author it again by hand."
            : "Runs in Docker, one task at a time, no model calls."}
        </span>
        {plan.live_reason && <span className="text-xs text-rose-300">{plan.live_reason}</span>}
        {error && <span className="text-xs text-rose-300">{error}</span>}
      </div>
    </div>
  );
}
