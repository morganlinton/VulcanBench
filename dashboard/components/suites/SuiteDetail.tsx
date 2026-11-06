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
      <Steps data={data} />
      {draft && <PrPicker id={id} onScaffolded={refresh} />}
      <TaskTable id={id} data={data} draft={draft} onChange={refresh} />
      <FreezeCard data={data} onFrozen={refresh} />
    </div>
  );
}

// Where the suite is on the way from pull requests to a frozen, benchmarkable
// suite. The first step whose condition is not met yet is the current one.
function currentStep(data: SuiteDetailData): number {
  if (data.suite.status === "frozen") return 5;
  const live = data.tasks.filter((t) => !t.dropped);
  if (live.length === 0) return 1;
  if (!live.some((t) => t.stage === "validated" || t.accepted)) return 2;
  if (!live.some((t) => t.accepted)) return 3;
  return 4;
}

const STEPS = [
  { n: 1, title: "Pick pull requests", text: "Choose merged pull requests from your repository. Each becomes one task." },
  { n: 2, title: "Build the tasks", text: "A model writes an issue and hidden tests for each task, then a check confirms the task works." },
  { n: 3, title: "Review and accept", text: "Read each task, keep the good ones, drop the rest." },
  { n: 4, title: "Freeze", text: "Lock the accepted tasks so every benchmark run uses the same suite." },
];

function Steps({ data }: { data: SuiteDetailData }) {
  const cur = currentStep(data);
  return (
    <section className="rounded-2xl border border-white/10 p-5">
      <p className="text-sm text-zinc-300">
        An eval suite is a set of tasks a model is scored on. Each task here comes from one merged pull request in{" "}
        <span className="font-mono">{repoName(data.suite.repo)}</span>: the model gets the code as it was before that
        change and an issue describing the problem, and passes if its fix makes the hidden tests pass.
        Building one takes four steps, in order down this page.
      </p>
      <ol className="mt-4 grid gap-3 md:grid-cols-4">
        {STEPS.map((s) => {
          const state = s.n < cur ? "done" : s.n === cur ? "now" : "later";
          const cls = state === "done"
            ? "border-emerald-500/30 text-zinc-400"
            : state === "now"
              ? "border-emerald-500/60 bg-emerald-500/[0.06] text-zinc-100"
              : "border-white/10 text-zinc-500";
          return (
            <li key={s.n} className={`rounded-xl border p-3 ${cls}`}>
              <div className="flex items-center gap-2 text-sm">
                <span className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-xs ${state === "done" ? "border-emerald-500/50 text-emerald-300" : state === "now" ? "border-emerald-400 bg-emerald-500 text-black" : "border-white/20"}`}>
                  {state === "done" ? "✓" : s.n}
                </span>
                <span className={state === "now" ? "font-medium" : ""}>{s.title}</span>
              </div>
              {state === "now" && <div className="mt-1 text-[10px] uppercase tracking-wider text-emerald-300">you are here</div>}
              <div className="mt-1.5 text-xs leading-relaxed">{s.text}</div>
            </li>
          );
        })}
      </ol>
      {cur === 5 && (
        <p className="mt-3 text-xs text-emerald-300">All four steps are done. This suite is frozen and ready to benchmark from the Launch page.</p>
      )}
    </section>
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

// The last loaded list per suite, kept in memory for this window so opening a
// task and coming back does not empty the table (a full reload starts over).
// The terms note typed for a suite, kept while the app stays open.
const lastLicenseNotes = new Map<string, string>();
const lastPrLists = new Map<string, { since: string; limit: number; list: PrList }>();

function PrPicker({ id, onScaffolded }: { id: string; onScaffolded: () => void }) {
  const { handle } = useGate();
  const kept = lastPrLists.get(id);
  const [since, setSince] = useState(() => kept?.since ?? ninetyDaysAgo());
  const [limit, setLimit] = useState(kept?.limit ?? 50);
  const [list, setListState] = useState<PrList | null>(kept?.list ?? null);
  const setList = (l: PrList) => {
    lastPrLists.set(id, { since, limit, list: l });
    setListState(l);
  };
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ text: string; gh: boolean } | null>(null);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [progress, setProgress] = useState<Record<number, RowState>>({});
  const [warnings, setWarnings] = useState<Record<number, string[]>>({});
  const [licenseNote, setLicenseNote] = useState(() => lastLicenseNotes.get(id) ?? "");
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
    setWarnings({});
    lastLicenseNotes.set(id, licenseNote);
    for (const n of prs) {
      setProgress((p) => ({ ...p, [n]: "running" }));
      try {
        const row = await runnerPost<TaskRow>(`${suitePath(id)}/tasks`, { pr: n, license_note: licenseNote.trim() });
        setProgress((p) => ({ ...p, [n]: "done" }));
        setWarnings((w) => ({ ...w, [n]: row.scaffold_warnings ?? [] }));
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
  // Pull requests that are not a task yet, the ones a checkbox can pick.
  const open = (list?.prs ?? []).filter((pr) => !pr.task_id).map((pr) => pr.number);
  const allOpenPicked = open.length > 0 && open.every((n) => picked.has(n));

  return (
    <section className="rounded-2xl border border-white/10 p-5 space-y-4">
      <div>
        <h2 className="text-lg font-medium"><span className="mr-2 text-zinc-500">1</span>Pick pull requests</h2>
        <p className="mt-1 text-xs text-zinc-400 max-w-3xl">
          Every task starts from a merged pull request. The best ones fix one thing, are small enough to review,
          and come with tests: the score below rates each on those points, and the Why column says what it found.
          Tick the ones you want and click Scaffold. That copies the repository as it was before each fix and keeps
          the fix itself as the answer key. It makes no model call and spends nothing.
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
                  <th className="px-3 py-2">
                    {open.length > 1 && (
                      <input type="checkbox" checked={allOpenPicked} onChange={() => setPicked(allOpenPicked ? new Set() : new Set(open))} disabled={running} aria-label="select every pull request" />
                    )}
                  </th>
                  <th className="px-3 py-2">PR</th>
                  <th className="px-3 py-2">Score</th>
                  <th className="px-3 py-2">Change</th>
                  <th className="px-3 py-2">Why</th>
                </tr>
              </thead>
              <tbody>
                {list.prs.map((pr) => (
                  <PrRow key={pr.number} pr={pr} suite={id} picked={picked.has(pr.number)} onToggle={() => toggle(pr.number)} state={progress[pr.number]} warnings={warnings[pr.number]} disabled={running} />
                ))}
              </tbody>
            </table>
          </div>
          <div className="max-w-2xl">
            <label className={label} htmlFor="license-note">Terms note (only if the repository has no LICENSE file)</label>
            <textarea
              id="license-note"
              rows={2}
              maxLength={2000}
              value={licenseNote}
              onChange={(e) => setLicenseNote(e.target.value)}
              disabled={running}
              placeholder="e.g. Internal code of Acme Inc., used only for Acme's own model evaluations."
              className={`${field} w-full`}
            />
            <p className="mt-1 text-xs text-zinc-500">
              Written to repo/NOTICE in each task scaffolded now. Validation requires a LICENSE or NOTICE, and a private
              repository often has neither. Leave it empty when the repository has a LICENSE.
            </p>
          </div>
          <button type="button" onClick={scaffold} disabled={picked.size === 0 || running} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
            {running ? "Scaffolding" : `Scaffold ${picked.size} selected`}
          </button>
          <span className="ml-3 text-xs text-zinc-500">Each scaffolded pull request shows up as a task in step 2.</span>
        </>
      )}
    </section>
  );
}

function PrRow({ pr, suite, picked, onToggle, state, warnings, disabled }: {
  pr: PullRequest;
  suite: string;
  picked: boolean;
  onToggle: () => void;
  state: RowState | undefined;
  warnings: string[] | undefined;
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
        {warnings?.map((w) => (
          <div key={w} className="text-xs text-amber-300">
            {w.startsWith("no_license") ? `no LICENSE or NOTICE in the repository, so validation will fail on provenance: add a NOTICE file with the terms to ${pr.task_id ?? "the task"}/repo/ in the suites home, and fill in the terms note below before scaffolding more.` : w}
          </div>
        ))}
      </td>
      <td className="px-3 py-2"><ScoreBar score={s.score} /></td>
      <td className="px-3 py-2 text-xs text-zinc-400 whitespace-nowrap">
        +{pr.additions} -{pr.deletions}, {pr.changed_files} {pr.changed_files === 1 ? "file" : "files"}, {pr.test_files.length} test {pr.test_files.length === 1 ? "file" : "files"}
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
  const selectableIds = shown.filter(selectable).map((t) => t.id);
  const allPicked = selectableIds.length > 0 && selectableIds.every((t) => picked.has(t));
  return (
    <section className="rounded-2xl border border-white/10 p-5 space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-medium"><span className="mr-2 text-zinc-500">2 and 3</span>Build and review the tasks</h2>
        {nDropped > 0 && (
          <button type="button" onClick={() => setShowDropped((v) => !v)} className="text-xs text-zinc-400 hover:text-white">
            {showDropped ? "hide" : "show"} {nDropped} dropped
          </button>
        )}
      </div>
      <div className="text-xs text-zinc-400 max-w-3xl space-y-1.5">
        <p>
          A task is one pull request turned into a test for a model: the code before the fix, an issue that
          describes the problem, hidden tests that pass only once it is fixed, and the original fix as the answer key.
          Each task goes through two steps, then you decide whether it belongs in the suite:
        </p>
        <ul className="list-disc pl-5 space-y-1">
          <li><span className="text-zinc-200">Author</span>: a model reads the fix and writes the issue and the hidden tests. This is the one step that spends model quota, one session per task, and it starts only after you confirm a plan that shows the cost.</li>
          <li><span className="text-zinc-200">Validate</span>: a check that the tests fail before the fix, pass after it, and give the same answer every time. No model call. A task must pass before it can be accepted.</li>
          <li><span className="text-zinc-200">Accept or Drop</span>: open a task to read what the model wrote. Accept the ones you want in the suite; drop the rest with a note on why.</li>
        </ul>
      </div>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      {shown.length === 0 ? (
        <div className="text-sm text-zinc-500">No tasks yet{draft ? ". Pick pull requests in step 1 and scaffold them; each one becomes a task here." : "."}</div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead className="text-left text-xs uppercase tracking-wider text-zinc-500">
              <tr>
                <th className="px-3 py-2">
                  {selectableIds.length > 1 && (
                    <input type="checkbox" checked={allPicked} onChange={() => setPicked(allPicked ? new Set() : new Set(selectableIds))} aria-label="select every task" />
                  )}
                </th>
                <th className="px-3 py-2">Task</th>
                <th className="px-3 py-2">Stage</th>
                <th className="px-3 py-2">Author</th>
                <th className="px-3 py-2">Validate</th>
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
                    ) : t.validate_stale ? (
                      <span className="text-amber-300">out of date: authored again since, validate again</span>
                    ) : <span className="text-zinc-500">not yet</span>}
                  </td>
                  <td className="px-3 py-2 text-xs">
                    {t.dropped ? <span className="text-zinc-500">dropped{t.dropped_reason ? `: ${t.dropped_reason}` : ""}</span> : t.accepted ? <span className="text-emerald-300">accepted</span> : <span className="text-zinc-500">no</span>}
                  </td>
                  <td className="px-3 py-2 text-xs whitespace-nowrap">
                    {draft && !t.dropped && (
                      <div className="flex gap-2">
                        {!t.accepted && (
                          <button type="button" onClick={() => setAsking({ task: t.id, what: "accept" })} disabled={busy !== null || t.stage !== "validated"} title={t.stage !== "validated" ? "Only a task whose validation passed can be accepted" : undefined} className="rounded-full border border-emerald-500/40 px-3 py-0.5 text-emerald-300 hover:bg-emerald-500/10 disabled:opacity-30">
                            Accept
                          </button>
                        )}
                        <button type="button" onClick={() => setAsking({ task: t.id, what: "drop" })} disabled={busy !== null || t.active_job !== null} title={t.active_job !== null ? "A job is working on this task; stop it or let it finish first" : undefined} className="rounded-full border border-white/15 px-3 py-0.5 text-zinc-300 hover:bg-white/5 disabled:opacity-30">
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
      {draft && shown.length > 0 && (
        <div className="space-y-3 border-t border-white/10 pt-4">
          <div className="flex flex-wrap items-center gap-2">
            <button type="button" onClick={() => setAction(action?.kind === "author" ? null : { kind: "author", tasks: authorable })} disabled={authorable.length === 0} className={`rounded-full border px-4 py-1 text-sm disabled:opacity-30 ${action?.kind === "author" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              Author {authorable.length || ""} selected
            </button>
            <button type="button" onClick={() => setAction(action?.kind === "validate" ? null : { kind: "validate", tasks: validatable })} disabled={validatable.length === 0} className={`rounded-full border px-4 py-1 text-sm disabled:opacity-30 ${action?.kind === "validate" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              Validate {validatable.length || ""} selected
            </button>
            <span className="text-xs text-zinc-500">Tick tasks in the table first. Author comes before Validate. Either shows a plan to read before anything is queued; only Author spends quota.</span>
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
        <p className="text-xs text-zinc-400 max-w-3xl">
          This suite is locked: its tasks cannot change, so every run scores the same thing. Each run checks the
          tasks against this lock digest before any model call and records it in its results.
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
      <h2 className="text-lg font-medium"><span className="mr-2 text-zinc-500">4</span>Freeze the suite</h2>
      <p className="text-xs text-zinc-400 max-w-3xl">
        Freezing locks in the accepted tasks and gives the suite a checksum, so every benchmark run uses exactly the
        same tasks and results can be compared across models and over time. Tasks that are not accepted are left
        out. After this the suite is read-only and appears on the Launch page under Custom, ready to benchmark.
        For more tasks later, start a new suite.
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
