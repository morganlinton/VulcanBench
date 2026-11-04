"use client";

import Link from "next/link";
import { Fragment, useState } from "react";
import { Row, button, field, label } from "@/components/runner/launchShared";
import { usePolling } from "@/components/runner/usePolling";
import { RunnerError, fmtDuration, runnerGet, runnerPost } from "@/lib/runner";
import {
  plural,
  suitePath,
  taskPath,
  type FreezeResult,
  type PrList,
  type PullRequest,
  type SuiteDetail as SuiteDetailData,
  type TaskRow,
} from "@/lib/suites";
import { ErrorLine, useGate } from "./BuilderGate";
import { BuildActions, lastAuthorChoice } from "./BuildPlan";
import { ConfirmBar } from "./ConfirmBar";
import { StageBadge, repoName } from "./SuitesList";

export function SuiteDetail({ id }: { id: string }) {
  const { data, error, refresh } = usePolling<SuiteDetailData>(
    () => runnerGet<SuiteDetailData>(suitePath(id)),
    4000,
    id,
  );
  if (error && !data) return <ErrorLine error={error} />;
  if (!data) return <div className="mt-6 text-sm text-zinc-500">loading suite</div>;
  const s = data.suite;
  const draft = s.status === "draft";
  return (
    <div className="mt-4 space-y-6">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <h1 className="text-3xl font-semibold tracking-tight">{s.name}</h1>
          <div className="mt-1 text-xs text-zinc-500">
            <span className="font-mono">{s.id}</span> from{" "}
            <a href={`https://github.com/${repoName(s.repo)}`} target="_blank" rel="noopener noreferrer" className="font-mono text-zinc-300 hover:underline">
              {repoName(s.repo)}
            </a>{" "}
            at <span className="font-mono">{s.path}</span>
          </div>
        </div>
        <div className="text-sm">
          {draft ? <span className="text-amber-300">draft</span> : <span className="text-zinc-200">frozen {s.version}, lock {s.lock_digest}</span>}
        </div>
      </div>
      {error && <ErrorLine error={error} />}
      {!data.tooling.ready && (
        <div className="rounded-xl border border-amber-500/40 p-3 text-xs text-amber-200">
          {data.tooling.problems.join("; ")}
        </div>
      )}
      {draft && <PrPicker id={id} onScaffolded={refresh} />}
      <TaskTable id={id} data={data} draft={draft} onChange={refresh} />
      <FreezeCard data={data} onFrozen={refresh} />
    </div>
  );
}

function ScoreBar({ score }: { score: number }) {
  const pct = Math.max(0, Math.min(100, score));
  const cls = pct >= 70 ? "bg-emerald-400" : pct >= 40 ? "bg-amber-400" : "bg-rose-400";
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-16 rounded bg-white/10"><div className={`h-1.5 rounded ${cls}`} style={{ width: `${pct}%` }} /></div>
      <span className="font-mono text-xs text-zinc-300">{score}</span>
    </div>
  );
}

function ninetyDaysAgo(): string {
  return new Date(Date.now() - 90 * 86400_000).toISOString().slice(0, 10);
}

type RowState = "waiting" | "running" | "done" | string;

function PrPicker({ id, onScaffolded }: { id: string; onScaffolded: () => void }) {
  const { handle } = useGate();
  const [since, setSince] = useState(ninetyDaysAgo);
  const [limit, setLimit] = useState(50);
  const [list, setList] = useState<PrList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ text: string; gh: boolean } | null>(null);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [progress, setProgress] = useState<Record<number, RowState>>({});
  const [running, setRunning] = useState(false);

  const load = async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      const qs = new URLSearchParams({ limit: String(limit) });
      if (since) qs.set("since", since);
      if (refresh) qs.set("refresh", "true");
      setList(await runnerGet<PrList>(`${suitePath(id)}/prs?${qs.toString()}`));
      setPicked(new Set());
    } catch (e) {
      const gh = e instanceof RunnerError && (e.code === "gh_auth" || e.code === "gh_missing");
      setError({ text: handle(e), gh });
    } finally {
      setLoading(false);
    }
  };

  // Sequential on purpose: one clone and slice at a time, each with its own row status.
  const scaffold = async () => {
    const prs = [...picked];
    setRunning(true);
    setProgress(Object.fromEntries(prs.map((n) => [n, "waiting"])));
    for (const n of prs) {
      setProgress((p) => ({ ...p, [n]: "running" }));
      try {
        await runnerPost<TaskRow>(`${suitePath(id)}/tasks`, { pr: n });
        setProgress((p) => ({ ...p, [n]: "done" }));
        onScaffolded();
      } catch (e) {
        setProgress((p) => ({ ...p, [n]: handle(e) }));
      }
    }
    setRunning(false);
    setPicked(new Set());
    await load(false);
  };

  const toggle = (n: number) =>
    setPicked((s) => {
      const next = new Set(s);
      if (next.has(n)) next.delete(n);
      else next.add(n);
      return next;
    });

  return (
    <section className="rounded-2xl border border-white/10 p-5 space-y-4">
      <div>
        <h2 className="text-lg font-medium">Pull requests</h2>
        <p className="text-xs text-zinc-400">
          Merged pull requests, scored as task candidates: tests in the change, a source change of a reviewable
          size, one purpose, a description. Scaffolding clones the repository at the change&apos;s base commit,
          writes a draft issue and the gold patch, and makes no model call.
        </p>
      </div>
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label className={label} htmlFor="pr-since">Merged since</label>
          <input id="pr-since" type="date" className={field} value={since} onChange={(e) => setSince(e.target.value)} />
        </div>
        <div>
          <label className={label} htmlFor="pr-limit">At most</label>
          <input id="pr-limit" type="number" min={1} max={200} className={`${field} w-24`} value={limit} onChange={(e) => setLimit(Math.max(1, Math.min(200, Number(e.target.value) || 50)))} />
        </div>
        <button type="button" onClick={() => load(false)} disabled={loading || running} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
          {loading ? "Loading" : "Load pull requests"}
        </button>
        {list && (
          <button type="button" onClick={() => load(true)} disabled={loading || running} className="text-xs text-zinc-400 hover:text-white">
            refresh{list.cached_s > 0 ? ` (fetched ${Math.round(list.cached_s / 60)} min ago)` : ""}
          </button>
        )}
      </div>
      {error && (
        <div className={`rounded-lg border p-3 text-xs ${error.gh ? "border-amber-500/40 text-amber-200" : "border-rose-500/40 text-rose-300"}`}>
          {error.text}
          {error.gh && <div className="mt-1 text-amber-200/80">gh must be installed and logged in on this Mac; run gh auth login in a terminal, then load again.</div>}
        </div>
      )}
      {list && list.prs.length === 0 && <div className="text-sm text-zinc-500">No merged pull requests match.</div>}
      {list && list.prs.length > 0 && (
        <>
          <div className="overflow-x-auto rounded-xl border border-white/10">
            <table className="w-full text-sm">
              <thead className="text-left text-xs uppercase tracking-wider text-zinc-500">
                <tr>
                  <th className="px-3 py-2" />
                  <th className="px-3 py-2">PR</th>
                  <th className="px-3 py-2">Score</th>
                  <th className="px-3 py-2">Change</th>
                  <th className="px-3 py-2">Why</th>
                </tr>
              </thead>
              <tbody>
                {list.prs.map((pr) => (
                  <PrRow key={pr.number} pr={pr} suite={id} picked={picked.has(pr.number)} onToggle={() => toggle(pr.number)} state={progress[pr.number]} disabled={running} />
                ))}
              </tbody>
            </table>
          </div>
          <button type="button" onClick={scaffold} disabled={picked.size === 0 || running} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
            {running ? "Scaffolding" : `Scaffold ${picked.size} selected`}
          </button>
        </>
      )}
    </section>
  );
}

function PrRow({ pr, suite, picked, onToggle, state, disabled }: {
  pr: PullRequest;
  suite: string;
  picked: boolean;
  onToggle: () => void;
  state: RowState | undefined;
  disabled: boolean;
}) {
  const s = pr.suitability;
  return (
    <tr className="border-t border-white/5 align-top">
      <td className="px-3 py-2">
        {pr.task_id ? null : <input type="checkbox" checked={picked} onChange={onToggle} disabled={disabled} aria-label={`select PR ${pr.number}`} />}
      </td>
      <td className="px-3 py-2">
        <a href={pr.url} target="_blank" rel="noopener noreferrer" className="text-zinc-200 hover:underline">#{pr.number} {pr.title}</a>
        <div className="text-xs text-zinc-500">merged {pr.merged_at?.slice(0, 10) ?? "?"}{pr.languages.length ? `, ${pr.languages.join(", ")}` : ""}</div>
        {pr.task_id && (
          <Link href={`/suites/${encodeURIComponent(suite)}/tasks/${encodeURIComponent(pr.task_id)}`} className="text-xs text-emerald-300 hover:underline">task {pr.task_id}</Link>
        )}
        {state && (
          <div className={`text-xs ${state === "done" ? "text-emerald-300" : state === "running" || state === "waiting" ? "text-sky-300" : "text-rose-300"}`}>
            {state === "running" ? "scaffolding" : state}
          </div>
        )}
      </td>
      <td className="px-3 py-2"><ScoreBar score={s.score} /></td>
      <td className="px-3 py-2 text-xs text-zinc-400 whitespace-nowrap">
        +{pr.additions} -{pr.deletions}, {pr.changed_files} files, {pr.test_files.length} test files
      </td>
      <td className="px-3 py-2 text-xs">
        <div className="text-zinc-400">{s.reasons.join("; ")}</div>
        {s.flags.length > 0 && <div className="text-amber-300">{s.flags.join(", ")}</div>}
      </td>
    </tr>
  );
}

function TaskTable({ id, data, draft, onChange }: { id: string; data: SuiteDetailData; draft: boolean; onChange: () => void }) {
  const { handle } = useGate();
  const tasks = data.tasks;
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showDropped, setShowDropped] = useState(false);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  // The tasks are fixed when the button is clicked, so the plan and the queued
  // screen survive the table refreshing underneath them.
  const [action, setAction] = useState<{ kind: "author" | "validate"; tasks: string[] } | null>(null);
  const selectable = (t: TaskRow) => draft && !t.dropped && t.active_job === null;
  const pickedTasks = tasks.filter((t) => picked.has(t.id) && selectable(t));
  const authorable = pickedTasks.map((t) => t.id);
  const validatable = pickedTasks.filter((t) => t.author?.status === "ok").map((t) => t.id);
  const toggle = (task: string) =>
    setPicked((s) => {
      const next = new Set(s);
      if (next.has(task)) next.delete(task);
      else next.add(task);
      return next;
    });
  // Accept and drop ask first, in the row itself (no native dialogs).
  const [asking, setAsking] = useState<{ task: string; what: "accept" | "drop" } | null>(null);
  const act = async (task: string, what: "accept" | "drop", reason: string) => {
    setBusy(task);
    setError(null);
    try {
      await runnerPost<TaskRow>(`${taskPath(id, task)}/${what}`, what === "drop" ? { reason } : undefined);
      setAsking(null);
      onChange();
    } catch (e) {
      setError(handle(e));
    } finally {
      setBusy(null);
    }
  };
  const shown = tasks.filter((t) => showDropped || !t.dropped);
  const nDropped = tasks.length - tasks.filter((t) => !t.dropped).length;
  return (
    <section className="rounded-2xl border border-white/10 p-5 space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-medium">Tasks</h2>
        {nDropped > 0 && (
          <button type="button" onClick={() => setShowDropped((v) => !v)} className="text-xs text-zinc-400 hover:text-white">
            {showDropped ? "hide" : "show"} {nDropped} dropped
          </button>
        )}
      </div>
      <p className="text-xs text-zinc-400">
        A task is scaffolded from a pull request, then a model writes its issue and hidden tests (authoring,
        one session behind a confirm click), then the validator checks that the base fails, the gold patch
        passes and both are deterministic. Accept the tasks that pass, then freeze the suite.
      </p>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      {shown.length === 0 ? (
        <div className="text-sm text-zinc-500">No tasks yet{draft ? ": load pull requests above and scaffold some." : "."}</div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead className="text-left text-xs uppercase tracking-wider text-zinc-500">
              <tr>
                <th className="px-3 py-2" />
                <th className="px-3 py-2">Task</th>
                <th className="px-3 py-2">Stage</th>
                <th className="px-3 py-2">Authoring</th>
                <th className="px-3 py-2">Validation</th>
                <th className="px-3 py-2">In suite</th>
                <th className="px-3 py-2" />
              </tr>
            </thead>
            <tbody>
              {shown.map((t) => (
                <Fragment key={t.id}>
                <tr className={`border-t border-white/5 align-top ${t.dropped ? "opacity-50" : ""}`}>
                  <td className="px-3 py-2">
                    {selectable(t) && (
                      <input type="checkbox" checked={picked.has(t.id)} onChange={() => toggle(t.id)} aria-label={`select ${t.id}`} />
                    )}
                  </td>
                  <td className="px-3 py-2">
                    <Link href={`/suites/${encodeURIComponent(id)}/tasks/${encodeURIComponent(t.id)}`} className="text-emerald-300 hover:underline">{t.id}</Link>
                    <div className="text-xs text-zinc-500">{t.pr ? `PR #${t.pr}` : ""}{t.title ? ` ${t.title}` : ""}</div>
                  </td>
                  <td className="px-3 py-2">
                    <StageBadge stage={t.stage} />
                    {t.active_job && (
                      <Link href={`/runs/${encodeURIComponent(t.active_job.run_id)}`} className="ml-2 text-xs text-emerald-300 hover:underline">job {t.active_job.id}</Link>
                    )}
                  </td>
                  <td className="px-3 py-2 text-xs text-zinc-400">
                    {t.author ? (
                      <>
                        <div className={t.author.status === "ok" ? "text-zinc-200" : "text-rose-300"}>
                          {t.author.status} {t.author.harness}:{t.author.model}{t.author.effort ? ` ${t.author.effort}` : ""}
                        </div>
                        <div>
                          {t.author.duration_s ? fmtDuration(t.author.duration_s) : ""}
                          {t.author.cost_usd != null ? `, $${t.author.cost_usd.toFixed(2)} reported` : ""}
                          {t.author.rejected ? `, ${t.author.rejected} ${t.author.rejected === 1 ? "change" : "changes"} discarded` : ""}
                        </div>
                      </>
                    ) : "not yet"}
                  </td>
                  <td className="px-3 py-2 text-xs">
                    {t.validate ? (
                      <>
                        <div className={t.validate.status === "PASS" || t.validate.status === "WARN" ? "text-emerald-300" : "text-rose-300"}>{t.validate.status}</div>
                        {Object.entries(t.validate.checks).filter(([, ok]) => ok === false).map(([k]) => (
                          <div key={k} className="text-rose-300">{k}</div>
                        ))}
                        {t.validate.reasons.slice(0, 2).map((r) => <div key={r} className="text-zinc-500">{r}</div>)}
                      </>
                    ) : <span className="text-zinc-500">not yet</span>}
                  </td>
                  <td className="px-3 py-2 text-xs">
                    {t.dropped ? <span className="text-zinc-500">dropped{t.dropped_reason ? `: ${t.dropped_reason}` : ""}</span> : t.accepted ? <span className="text-emerald-300">accepted</span> : <span className="text-zinc-500">no</span>}
                  </td>
                  <td className="px-3 py-2 text-xs whitespace-nowrap">
                    {draft && !t.dropped && (
                      <div className="flex gap-2">
                        {!t.accepted && (
                          <button type="button" onClick={() => setAsking({ task: t.id, what: "accept" })} disabled={busy !== null || t.stage !== "validated"} className="rounded-full border border-emerald-500/40 px-3 py-0.5 text-emerald-300 hover:bg-emerald-500/10 disabled:opacity-30">
                            Accept
                          </button>
                        )}
                        <button type="button" onClick={() => setAsking({ task: t.id, what: "drop" })} disabled={busy !== null || t.active_job !== null} className="rounded-full border border-white/15 px-3 py-0.5 text-zinc-300 hover:bg-white/5 disabled:opacity-30">
                          Drop
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
                {asking?.task === t.id && (
                  <tr>
                    <td colSpan={7} className="px-3 pb-3">
                      <ConfirmBar
                        message={asking.what === "accept"
                          ? `Accept ${t.id} into the suite? It is frozen with the suite later.`
                          : `Drop ${t.id}? It leaves the suite; its folder stays in the suites home.`}
                        confirmLabel={asking.what === "accept" ? "Accept" : "Drop"}
                        withReason={asking.what === "drop"}
                        busy={busy === t.id}
                        onConfirm={(reason) => act(t.id, asking.what, reason)}
                        onCancel={() => setAsking(null)}
                      />
                    </td>
                  </tr>
                )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {draft && (
        <div className="space-y-3 border-t border-white/10 pt-4">
          <div className="flex flex-wrap items-center gap-2">
            <button type="button" onClick={() => setAction(action?.kind === "author" ? null : { kind: "author", tasks: authorable })} disabled={authorable.length === 0} className={`rounded-full border px-4 py-1 text-sm disabled:opacity-30 ${action?.kind === "author" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              Author {authorable.length || ""} selected
            </button>
            <button type="button" onClick={() => setAction(action?.kind === "validate" ? null : { kind: "validate", tasks: validatable })} disabled={validatable.length === 0} className={`rounded-full border px-4 py-1 text-sm disabled:opacity-30 ${action?.kind === "validate" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              Validate {validatable.length || ""} selected
            </button>
            <span className="text-xs text-zinc-500">Validation needs a successful authoring session first. Both build a plan to read before anything is queued.</span>
          </div>
          {action && (
            <BuildActions
              key={`${action.kind}:${action.tasks.join(",")}`}
              suite={id}
              tasks={action.tasks}
              options={data.options}
              kind={action.kind}
              defaults={lastAuthorChoice(tasks.filter((t) => action.tasks.includes(t.id)).concat(tasks))}
              onDone={() => { setAction(null); setPicked(new Set()); onChange(); }}
            />
          )}
        </div>
      )}
    </section>
  );
}

function FreezeCard({ data, onFrozen }: { data: SuiteDetailData; onFrozen: () => void }) {
  const { handle } = useGate();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<FreezeResult | null>(null);
  const [asking, setAsking] = useState(false);
  const s = data.suite;
  const launchHref = `/launch?suite=${encodeURIComponent(s.id)}`;
  if (s.status === "frozen") {
    return (
      <section className="rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.03] p-5 space-y-3">
        <h2 className="text-lg font-medium">Frozen</h2>
        <dl className="text-xs space-y-1 max-w-xl">
          <Row k="version" v={s.version ?? "?"} />
          <Row k="frozen" v={s.frozen_at ?? "?"} />
          <Row k="lock digest" v={s.lock_digest ?? "?"} />
          <Row k="tasks" v={String(s.n_accepted)} />
        </dl>
        <p className="text-xs text-zinc-400">
          A frozen suite is read-only here. Every run checks the tasks against this lock before any model call
          and records the digest in its summary.
        </p>
        <Link href={launchHref} className={`${button} inline-block bg-emerald-500 text-black hover:bg-emerald-400`}>Benchmark this suite</Link>
      </section>
    );
  }
  const leftOut = data.tasks.filter((t) => !t.accepted && !t.dropped).length;
  const freeze = async () => {
    setBusy(true);
    setError(null);
    try {
      setResult(await runnerPost<FreezeResult>(`${suitePath(s.id)}/freeze`));
      setAsking(false);
      onFrozen();
    } catch (e) {
      setError(handle(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="rounded-2xl border border-white/10 p-5 space-y-3">
      <h2 className="text-lg font-medium">Freeze</h2>
      <p className="text-xs text-zinc-400">
        Freezing writes <code className="font-mono">suite.lock.json</code> with the hash of every accepted task.
        After that the suite shows up on the Launch page under Custom.
      </p>
      {data.freeze_ready.problems.length > 0 && (
        <ul className="list-disc pl-5 text-xs text-amber-300">
          {data.freeze_ready.problems.map((p) => <li key={p}>{p}</li>)}
        </ul>
      )}
      {error && <div className="text-xs text-rose-300">{error}</div>}
      {result && <div className="text-xs text-emerald-300">froze {plural(result.n_tasks ?? 0, "task")}, lock {result.lock_digest}</div>}
      {asking ? (
        <ConfirmBar
          message={`Freeze ${s.name} with ${plural(s.n_accepted, "accepted task")}?${leftOut ? ` ${plural(leftOut, "task")} not accepted ${leftOut === 1 ? "is" : "are"} left out.` : ""} The suite becomes read-only and can then be benchmarked.`}
          confirmLabel="Freeze"
          busy={busy}
          onConfirm={freeze}
          onCancel={() => setAsking(false)}
        />
      ) : (
        <button type="button" onClick={() => setAsking(true)} disabled={!data.freeze_ready.ok || busy} className={`${button} border border-emerald-500/50 text-emerald-300 hover:bg-emerald-500/10`}>
          Freeze suite
        </button>
      )}
    </section>
  );
}
