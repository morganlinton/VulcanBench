"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";
import {
  commandLine,
  fmtDuration,
  fmtMoney,
  publicSuiteName,
  runnerPost,
  type JobSummary,
  type LaunchOptions,
  type LaunchPlan,
  type PreparePlanSpec,
  type SweepPlanSpec,
} from "@/lib/runner";
import { Countdown, Row, Warnings, WorktreeCard, button, field, label } from "./launchShared";

type Props = {
  opts: LaunchOptions;
  tag: string;
  onTag: (tag: string) => void;
  refreshOptions: () => void;
};

function DoctorLine({ d, harness }: { d: SweepPlanSpec["doctor"]; harness: string }) {
  if (d.status === "not_applicable") return <div className="text-xs text-zinc-400">{d.note}</div>;
  if (d.status === "ready") {
    return (
      <div className="text-xs text-emerald-300">
        harness doctor: {harness} {d.version} ready, {d.auth_mode}{d.plan_name ? ` (${d.plan_name})` : ""}. No model call was made.
      </div>
    );
  }
  return (
    <div className="text-xs text-rose-300">
      harness doctor: {d.status === "not_ready" ? `${harness} is not ready: ${d.detail ?? ""}` : d.note ?? d.status}
      {d.status === "not_ready" && " (confirm is disabled)"}
    </div>
  );
}

function PreparePanel({ tag, onQueued }: { tag: string; onQueued: () => void }) {
  const [plan, setPlan] = useState<LaunchPlan<PreparePlanSpec> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [queued, setQueued] = useState<JobSummary | null>(null);
  const build = async () => {
    setBusy(true);
    setError(null);
    try {
      setPlan(await runnerPost<LaunchPlan<PreparePlanSpec>>("/api/runner/plans", { kind: "prepare", tag }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const confirm = async () => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      setQueued(await runnerPost<JobSummary>(`/api/runner/plans/${plan.id}/confirm`, { hash: plan.hash }));
      onQueued();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  if (queued) {
    return (
      <div className="rounded-xl border border-emerald-500/40 p-3 text-xs">
        <span className="text-emerald-300">Queued as job {queued.id}: {queued.name}.</span>{" "}
        <span className="text-zinc-400">The supervisor runs run_worktree.sh when nothing else is running (make setup takes minutes). Come back and build the plan when the worktree card turns green.</span>{" "}
        <Link href="/runs" className="text-emerald-400 hover:underline">Watch it on Runs</Link>
      </div>
    );
  }
  return (
    <div className="rounded-xl border border-amber-500/30 p-3 text-xs space-y-2">
      <div className="text-zinc-300">The worktree for <span className="font-mono">{tag}</span> does not exist. Preparing it is a queue step with no model calls.</div>
      {!plan ? (
        <button type="button" onClick={build} disabled={busy} className={`${button} border border-amber-500/50 text-amber-300 hover:bg-amber-500/10`}>
          {busy ? "planning" : "Plan the worktree step"}
        </button>
      ) : (
        <div className="space-y-2">
          <pre className="rounded-lg bg-black/40 p-3 text-xs text-zinc-300 overflow-x-auto whitespace-pre-wrap break-all">{commandLine(plan.spec.prep_env, plan.spec.prep_argv)}</pre>
          <Warnings items={plan.spec.warnings} />
          <div className="flex items-center gap-3">
            <button type="button" onClick={confirm} disabled={busy} className={`${button} bg-amber-400 text-black hover:bg-amber-300`}>
              {busy ? "queueing" : "Confirm: prepare the worktree"}
            </button>
            <Countdown until={plan.expires_at} />
          </div>
        </div>
      )}
      {error && <div className="text-rose-300">{error}</div>}
    </div>
  );
}

function SweepPlanCard({ plan, onQueued }: { plan: LaunchPlan<SweepPlanSpec>; onQueued: (j: JobSummary) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const spec = plan.spec;
  const est = spec.estimate;
  const doctorBlocks = spec.doctor.status === "not_ready";
  const canConfirm = !plan.live_reason && !doctorBlocks && !busy;
  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      onQueued(await runnerPost<JobSummary>(`/api/runner/plans/${plan.id}/confirm`, { hash: plan.hash }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const shown = showAll ? spec.jobs : spec.jobs.slice(0, 1);
  return (
    <div className="mt-6 rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.03] p-5 space-y-5">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-semibold">Plan: <span className="font-mono text-emerald-300">{spec.name}</span></h2>
        <div className="text-xs"><Countdown until={plan.expires_at} /> <span className="text-zinc-600 ml-2 font-mono">hash {plan.hash}</span></div>
      </div>
      <Warnings items={spec.warnings} />
      <DoctorLine d={spec.doctor} harness={spec.harness} />

      <div>
        <div className={label}>Exactly what runs ({spec.jobs.length} queued job{spec.jobs.length > 1 ? "s" : ""}, one per effort level, one at a time)</div>
        {shown.map((j) => (
          <pre key={j.effort} className="mt-2 rounded-lg bg-black/40 p-3 text-xs text-zinc-200 overflow-x-auto whitespace-pre-wrap break-all">
            <span className="text-zinc-500">{`# ${j.effort}: cd ${j.cwd}\n`}</span>
            {commandLine(j.env, j.argv)}
          </pre>
        ))}
        {spec.jobs.length > 1 && (
          <button type="button" onClick={() => setShowAll((v) => !v)} className="mt-1 text-xs text-emerald-400 hover:underline">
            {showAll ? "show the first level only" : `show all ${spec.jobs.length} levels (only --effort and -o differ)`}
          </button>
        )}
        <div className="mt-1 text-xs text-zinc-500">
          child environment: {spec.env_passthrough.join(", ")} passed through{spec.path_prepend ? `, PATH starts with ${spec.path_prepend}` : ""}; nothing else (no VULCANBENCH_*, no keys or tokens). Provider keys, when metered, come from the worktree&apos;s .env.
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 text-sm">
        <div className="rounded-xl border border-white/10 p-4">
          <div className={label}>Estimate (vulcanbench estimate --json at this tag)</div>
          <dl className="space-y-1 text-xs">
            <Row k="runs" v={`${est.runs_per_level} per level, ${est.total_runs} in all (${spec.n_tasks} tasks x ${spec.repeat})`} />
            <Row k="list-price cost" v={est.status === "ok" ? `${fmtMoney(est.per_level_usd)} per level, ${fmtMoney(est.total_usd)} in all (${est.confidence} confidence)` : est.status === "no_pricing" ? "no pricing for this model at this tag" : (est.note ?? "unavailable")} mono={false} />
            {est.status === "ok" && est.per_level_low_usd !== null && (
              <Row k="range per level" v={`${fmtMoney(est.per_level_low_usd)} to ${fmtMoney(est.per_level_high_usd)}`} />
            )}
            <Row k="task timeout" v={`${fmtDuration(est.task_timeout_s)} per run`} />
            <Row k="longest wall time" v={fmtDuration(est.max_wall_s)} />
            <Row k="billing" v={est.billing} mono={false} />
            <Row k="retries" v={est.infra_retries} mono={false} />
            <Row k="quota windows" v={est.quota} mono={false} />
          </dl>
          {est.notes.length > 0 && <div className="mt-2 text-xs text-zinc-500">{est.notes.join(" ")}</div>}
        </div>
        <div className="rounded-xl border border-white/10 p-4">
          <div className={label}>Source</div>
          <dl className="space-y-1 text-xs">
            <Row k="suite" v={`${spec.suite_name} (${spec.suite})`} mono={false} />
            <Row k="model" v={spec.model_spec} />
            <Row k="harness" v={`${spec.harness}${spec.doctor.version ? ` ${spec.doctor.version}` : ""}`} />
            <Row k="effort levels" v={spec.efforts.join(", ")} />
            <Row k="sandbox" v={`${spec.sandbox}${spec.sandbox === spec.sandbox_default ? " (default for this harness)" : " (overridden)"}`} />
            <Row k="resume" v={spec.only_missing ? "--only-missing: tasks with a fresh result in the output dir are skipped" : "every task runs"} mono={false} />
            <Row k="bench tag" v={`${spec.tag} @ ${spec.tag_commit ?? "?"}`} />
            <Row k="output" v={spec.out_root} />
          </dl>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-4 border-t border-white/10 pt-4">
        <button type="button" onClick={confirm} disabled={!canConfirm} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
          {busy ? "queueing" : `Confirm and queue ${spec.jobs.length} level${spec.jobs.length > 1 ? "s" : ""}`}
        </button>
        <span className="text-xs text-zinc-500">
          Queues one job per level; the supervisor runs them one at a time. Nothing retries a failed task{spec.quota_continue.max > 0 ? `; only a quota window continues, at most ${spec.quota_continue.max} time(s)` : ""}.
        </span>
        {plan.live_reason && <span className="text-xs text-rose-300">{plan.live_reason}</span>}
        {error && <span className="text-xs text-rose-300">{error}</span>}
      </div>
    </div>
  );
}

export function SweepForm({ opts, tag, onTag, refreshOptions }: Props) {
  const router = useRouter();
  const params = useSearchParams();
  const [model, setModel] = useState(params.get("model") ?? "");
  const [harness, setHarness] = useState(params.get("harness") ?? "codex");
  const [billing, setBilling] = useState(params.get("billing") ?? "auto");
  const [suiteChoice, setSuiteChoice] = useState<string | null>(params.get("suite"));
  const [efforts, setEfforts] = useState<string[]>(() => {
    const e = params.get("efforts");
    return e ? e.split(",").filter(Boolean) : ["low", "medium", "high", "extra-high", "max"];
  });
  const [repeat, setRepeat] = useState(1);
  const [onlyMissing, setOnlyMissing] = useState(true);
  const [sandbox, setSandbox] = useState<string>("");
  const [quota, setQuota] = useState(0);
  const [labelText, setLabelText] = useState(params.get("label") ?? "");
  const [binDir, setBinDir] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [plan, setPlan] = useState<LaunchPlan<SweepPlanSpec> | null>(null);
  const [building, setBuilding] = useState(false);
  const [buildError, setBuildError] = useState<string | null>(null);
  const [queued, setQueued] = useState<JobSummary | null>(null);

  const harnessName = opts.harnesses.includes(harness) ? harness : opts.harnesses[0];
  const suite = suiteChoice && opts.suites.some((s) => s.id === suiteChoice) ? suiteChoice : (opts.suites.find((s) => s.id === "coding-intelligence-index-v4")?.id ?? opts.suites[0]?.id ?? "");
  const suiteInfo = opts.suites.find((s) => s.id === suite);
  const chosenEfforts = opts.efforts.filter((e) => efforts.includes(e));
  const wt = opts.worktree;
  const wtReady = !!wt && wt.exists && wt.problems.length === 0;
  const formReady = !!tag && !!model.trim() && !!suite && chosenEfforts.length > 0 && wtReady;

  const toggleEffort = (e: string) => {
    setPlan(null);
    setEfforts((cur) => (cur.includes(e) ? cur.filter((x) => x !== e) : [...cur, e]));
  };
  const build = async () => {
    setBuilding(true);
    setBuildError(null);
    setPlan(null);
    try {
      setPlan(
        await runnerPost<LaunchPlan<SweepPlanSpec>>("/api/runner/plans", {
          kind: "sweep",
          model: model.trim(),
          harness: harnessName,
          suite,
          tag,
          efforts: chosenEfforts,
          billing,
          repeat,
          only_missing: onlyMissing,
          sandbox: sandbox || null,
          quota_continue: quota,
          label: labelText.trim(),
          cli_bin_dir: binDir.trim(),
        }),
      );
    } catch (e) {
      setBuildError(e instanceof Error ? e.message : String(e));
    } finally {
      setBuilding(false);
    }
  };

  if (queued) {
    const jobs = queued.jobs ?? [queued];
    return (
      <div className="mt-6 rounded-2xl border border-emerald-500/40 p-5 text-sm">
        <div className="text-emerald-300 font-medium">Queued {jobs.length} job{jobs.length > 1 ? "s" : ""}: <span className="font-mono">{jobs.map((j) => j.name).join(", ")}</span></div>
        <div className="mt-1 text-zinc-400">The supervisor runs them one at a time when nothing else is running. Progress per level is on the Runs page.</div>
        <div className="mt-3 flex gap-3">
          <button type="button" onClick={() => router.push("/runs")} className={`${button} bg-emerald-500 text-black`}>Go to runs</button>
          <button type="button" onClick={() => { setQueued(null); setPlan(null); }} className={`${button} border border-white/15 text-zinc-200`}>Plan another</button>
        </div>
      </div>
    );
  }

  const reset = () => setPlan(null);
  return (
    <div className="mt-6 space-y-6">
      <div className="rounded-2xl border border-white/10 p-5 space-y-5">
        <div className="grid gap-4 md:grid-cols-3">
          <div className="md:col-span-1">
            <label className={label} htmlFor="model">Model id</label>
            <input id="model" className={field} placeholder={harnessName === "vulcan" ? "openai:gpt-6.1-terra" : "gpt-6.1-terra"} value={model} onChange={(e) => { setModel(e.target.value); reset(); }} />
            <div className="mt-1 text-xs text-zinc-500">{harnessName === "vulcan" ? "provider:model for Vulcan's API loop" : "the id the harness CLI takes; the harness is the provider"}</div>
          </div>
          <div>
            <label className={label} htmlFor="harness">Harness</label>
            <select id="harness" className={field} value={harnessName} onChange={(e) => { setHarness(e.target.value); setSandbox(""); reset(); }}>
              {opts.harnesses.map((h) => <option key={h} value={h}>{h}</option>)}
            </select>
          </div>
          <div>
            <label className={label} htmlFor="billing">Billing</label>
            <select id="billing" className={field} value={billing} onChange={(e) => { setBilling(e.target.value); reset(); }}>
              {opts.billing.map((b) => <option key={b} value={b}>{b}</option>)}
            </select>
            <div className="mt-1 text-xs text-zinc-500">auto: the subscription login when the harness has one, else the API key</div>
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <label className={label} htmlFor="suite">Suite</label>
            <select id="suite" className={field} value={suite} onChange={(e) => { setSuiteChoice(e.target.value); reset(); }}>
              {opts.suites.map((s) => <option key={s.id} value={s.id}>{publicSuiteName(s.id) ?? s.name} ({s.n_tasks} tasks, {s.id})</option>)}
            </select>
            {opts.suites.length === 0 && <div className="mt-1 text-xs text-amber-300">No frozen suite (tasks/&lt;suite&gt;/suite.lock.json) at this tag.</div>}
            {suiteInfo && <div className="mt-1 text-xs text-zinc-500">frozen {suiteInfo.frozen_at ?? "?"}, lock version {suiteInfo.version ?? "?"}</div>}
          </div>
          <div>
            <label className={label} htmlFor="tag">Bench tag</label>
            <select id="tag" className={field} value={tag} onChange={(e) => { onTag(e.target.value); setSuiteChoice(null); reset(); }}>
              {opts.tags.map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
            <div className="mt-2">{wt && <WorktreeCard wt={wt} />}</div>
            {wt && !wt.exists && <div className="mt-2"><PreparePanel tag={tag} onQueued={refreshOptions} /></div>}
          </div>
        </div>

        <div>
          <div className={label}>Effort levels (one queued job each; blocked levels are never offered)</div>
          <div className="flex flex-wrap gap-2">
            {opts.efforts.map((e) => {
              const on = efforts.includes(e);
              return (
                <button key={e} type="button" onClick={() => toggleEffort(e)} className={`rounded-full border px-3 py-1 text-sm ${on ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/10 text-zinc-400 hover:bg-white/5"}`}>
                  {e}
                </button>
              );
            })}
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-4">
          <div>
            <label className={label} htmlFor="repeat">Runs per task</label>
            <select id="repeat" className={field} value={repeat} onChange={(e) => { setRepeat(Number(e.target.value)); reset(); }}>
              {Array.from({ length: opts.max_repeat }, (_, i) => i + 1).map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </div>
          <div>
            <label className={label} htmlFor="quota">Continue after a quota window</label>
            <select id="quota" className={field} value={quota} onChange={(e) => { setQuota(Number(e.target.value)); reset(); }}>
              <option value={0}>never (default)</option>
              {[1, 2, 3, 5, 10, 24, 48].filter((n) => n <= opts.quota_continue_max).map((n) => <option key={n} value={n}>up to {n} time{n > 1 ? "s" : ""}</option>)}
            </select>
            <div className="mt-1 text-xs text-zinc-500">waits {Math.round(opts.quota_wait_s / 60)} min, then --only-missing; no other failure continues</div>
          </div>
          <div>
            <label className={label} htmlFor="sandbox">Sandbox</label>
            <select id="sandbox" className={field} value={sandbox} onChange={(e) => { setSandbox(e.target.value); reset(); }}>
              <option value="">{opts.sandbox_defaults[harnessName] ?? "docker"} (default for {harnessName})</option>
              {opts.sandboxes.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          </div>
          <div>
            <label className={label} htmlFor="label">Output label</label>
            <input id="label" className={field} placeholder="auto: model-suite-date" value={labelText} onChange={(e) => { setLabelText(e.target.value); reset(); }} />
            <div className="mt-1 text-xs text-zinc-500 break-all">{opts.harness_runs_dir}/&lt;label&gt;/&lt;level&gt;</div>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-6 text-sm">
          <label className="flex items-center gap-2 text-zinc-300">
            <input type="checkbox" checked={onlyMissing} onChange={(e) => { setOnlyMissing(e.target.checked); reset(); }} />
            --only-missing: skip tasks that already have a fresh result in the output directory (resume)
          </label>
          <button type="button" onClick={() => setAdvanced((v) => !v)} className="text-xs text-zinc-500 hover:text-zinc-300">{advanced ? "hide" : "advanced"}</button>
        </div>
        {advanced && (
          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <label className={label} htmlFor="bindir">Pinned CLI directory (prepended to PATH)</label>
              <input id="bindir" className={field} placeholder="~/.local/share/vulcanbench/codex-0.159.0/node_modules/.bin" value={binDir} onChange={(e) => { setBinDir(e.target.value); reset(); }} />
              <div className="mt-1 text-xs text-zinc-500">for a model that needs a newer or pinned harness CLI than the one on PATH</div>
            </div>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-4 border-t border-white/10 pt-4">
          <button type="button" onClick={build} disabled={!formReady || building} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
            {building ? "building plan (doctor and estimate run, no model call)" : "Build plan"}
          </button>
          <span className="text-xs text-zinc-500">Validates the worktree, runs the harness doctor and the cost estimate. Nothing starts yet.</span>
          {!wtReady && tag && <span className="text-xs text-amber-300">the worktree must exist and be clean before a plan can be built</span>}
          {buildError && <span className="text-xs text-rose-300">{buildError}</span>}
        </div>
      </div>

      {plan && <SweepPlanCard plan={plan} onQueued={setQueued} />}

      <p className="text-xs text-zinc-600">
        Every level runs <span className="font-mono">vulcanbench run</span> from a worktree under <span className="font-mono">{opts.run_worktrees}</span> pinned to its bench tag, never from the working checkout and never with --allow-dirty. Output goes to <span className="font-mono">{opts.harness_runs_dir}</span>. Onboarding a new model (list price, pinned CLI, decision entry) still happens in a PR before a tag is cut.
      </p>
    </div>
  );
}
