"use client";

import Link from "next/link";
import { useState } from "react";
import { usePolling } from "@/components/runner/usePolling";
import { fmtDuration, runnerGet, runnerPost } from "@/lib/runner";
import { prUrl, taskPath, type TaskDetail, type TaskRow, type TextFile } from "@/lib/suites";
import { LogTail } from "@/components/runner/LogTail";
import { ErrorLine, useGate } from "./BuilderGate";
import { BuildActions, lastAuthorChoice } from "./BuildPlan";
import { ConfirmBar } from "./ConfirmBar";
import { StageBadge } from "./SuitesList";

const TABS = ["Issue", "Tests", "Gold patch", "Metadata", "Author", "Validate"] as const;
type Tab = (typeof TABS)[number];

function Text({ file, empty }: { file: TextFile | null | undefined; empty: string }) {
  if (!file) return <div className="text-sm text-zinc-500">{empty}</div>;
  if (file.binary || file.text === null) return <div className="text-sm text-zinc-500">binary file, {file.size} bytes</div>;
  return (
    <div>
      <pre className="max-h-[32rem] overflow-auto whitespace-pre-wrap rounded-lg bg-zinc-900 p-3 font-mono text-xs text-zinc-200">{file.text}</pre>
      {file.truncated && <div className="mt-1 text-xs text-amber-300">truncated: {file.size} bytes on disk</div>}
    </div>
  );
}

function Json({ value, empty }: { value: unknown; empty: string }) {
  if (value === null || value === undefined) return <div className="text-sm text-zinc-500">{empty}</div>;
  return <pre className="max-h-[32rem] overflow-auto rounded-lg bg-zinc-900 p-3 font-mono text-xs text-zinc-200">{JSON.stringify(value, null, 2)}</pre>;
}

// What to do after each outcome, so a failed step never ends on a bare badge.
const NEXT_STEP: Partial<Record<TaskRow["stage"], string>> = {
  scaffolded: "Next: author it, which writes the issue and hidden tests (one model session).",
  authored: "Next: validate it in Docker (no model calls), then read the issue and tests before accepting.",
  validated: "Next: read the issue, tests and gold patch, then accept it.",
  author_failed: "Author it again, or drop the task.",
  validation_failed: "Author it again to rewrite the issue and tests, or drop the task.",
};

function LatestJob({ task, jobs, frozen }: { task: TaskRow; jobs: TaskDetail["jobs"]; frozen: boolean }) {
  if (task.active_job) return null;
  const last = jobs[0];
  const failedStage = task.stage === "author_failed" || task.stage === "validation_failed";
  // A job that ended badly without writing its record leaves the task as it was;
  // say so rather than pointing at the next step of the older record.
  const broke = !!last && ["failed", "stopped", "lost", "infra_failed"].includes(last.status) && !failedStage;
  const next = task.dropped || frozen
    ? null
    : task.accepted
      ? "Accepted. Back on the eval suite page, review the other tasks, then freeze it to benchmark it."
      : broke
        ? `This job wrote no new ${last.kind === "author" ? "authoring" : "validation"} record, so the task keeps its earlier state (${task.stage.replace("_", " ")}). Read the job's log for the cause, then run it again.`
        : NEXT_STEP[task.stage];
  if (!last && !next) return null;
  const failed = failedStage || broke;
  const tone = failed ? "border-rose-500/30 bg-rose-500/5 text-rose-200" : "border-white/10 text-zinc-300";
  return (
    <div className={`rounded-lg border px-3 py-2 text-xs space-y-1 ${tone}`}>
      {last && (
        <div>
          Last job: {last.kind} {last.id} {last.status}
          {last.finished_at ? ` at ${new Date(last.finished_at).toLocaleString()}` : ""}
          {last.notes.length > 0 ? `: ${last.notes.join("; ")}` : ""}
          {failed && (
            <Link href={`/runs/${encodeURIComponent(last.run_id)}`} className="ml-2 text-emerald-300 hover:underline">open its log</Link>
          )}
        </div>
      )}
      {next && <div className="text-zinc-400">{next}</div>}
    </div>
  );
}

function Fact({ k, v, tone }: { k: string; v: React.ReactNode; tone?: string }) {
  return (
    <div className="flex gap-3">
      <dt className="w-28 shrink-0 text-zinc-500">{k}</dt>
      <dd className={tone ?? "text-zinc-200"}>{v}</dd>
    </div>
  );
}

function AuthorSummary({ task }: { task: TaskRow }) {
  const a = task.author;
  if (!a) return null;
  return (
    <dl className="space-y-1 rounded-lg border border-white/10 p-3 text-xs">
      <Fact k="result" v={a.status ?? "?"} tone={a.status === "ok" ? "text-emerald-300" : "text-rose-300"} />
      <Fact k="model" v={`${a.harness ?? "?"}:${a.model ?? "?"}${a.effort ? ` at ${a.effort}` : ""}`} />
      {a.finished_at && <Fact k="finished" v={new Date(a.finished_at).toLocaleString()} />}
      {a.duration_s != null && <Fact k="took" v={fmtDuration(a.duration_s)} />}
      {a.cost_usd != null && <Fact k="cost" v={`$${a.cost_usd.toFixed(2)} reported by the CLI`} />}
      <Fact k="changes" v={`${a.accepted} kept, ${a.rejected} discarded (only the issue, hidden tests and allowed metadata are kept)`} />
      {a.warnings.length > 0 && <Fact k="warnings" v={a.warnings.join("; ")} tone="text-amber-300" />}
    </dl>
  );
}

function ValidateSummary({ task, raw }: { task: TaskRow; raw: unknown }) {
  const v = task.validate;
  if (!v) {
    return raw ? (
      <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 text-xs text-amber-200">
        The validation record below is older than the last authoring session, so it no longer counts. Validate again.
      </div>
    ) : null;
  }
  const good = v.status === "PASS" || v.status === "WARN";
  return (
    <dl className="space-y-1 rounded-lg border border-white/10 p-3 text-xs">
      <Fact k="verdict" v={v.status ?? "?"} tone={good ? "text-emerald-300" : "text-rose-300"} />
      {v.reasons.length > 0 && <Fact k="why" v={v.reasons.join("; ")} tone={good ? "text-zinc-300" : "text-rose-200"} />}
      {Object.entries(v.checks).map(([name, ok]) => (
        <Fact
          key={name}
          k={name.replaceAll("_", " ")}
          v={ok === null ? "not checked" : ok ? "yes" : "no"}
          tone={ok === false ? "text-rose-300" : ok ? "text-emerald-300" : "text-zinc-500"}
        />
      ))}
      {v.sandbox && <Fact k="sandbox" v={v.sandbox} />}
      {v.finished_at && <Fact k="checked" v={new Date(v.finished_at).toLocaleString()} />}
    </dl>
  );
}

export function TaskReview({ suite, task }: { suite: string; task: string }) {
  const { handle } = useGate();
  const [tab, setTab] = useState<Tab>("Issue");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [build, setBuild] = useState<"author" | "validate" | null>(null);
  const [asking, setAsking] = useState<"accept" | "drop" | null>(null);
  const { data, error, refresh } = usePolling<TaskDetail>(
    () => runnerGet<TaskDetail>(taskPath(suite, task)),
    5000,
    `${suite}/${task}`,
  );
  if (error && !data) return <ErrorLine error={error} />;
  if (!data) return <div className="mt-6 text-sm text-zinc-500">loading task</div>;
  const t = data.task;
  const editable = !data.frozen && !t.dropped;

  const act = async (what: "accept" | "drop", reason: string) => {
    setBusy(true);
    setActionError(null);
    try {
      await runnerPost<TaskRow>(`${taskPath(suite, task)}/${what}`, what === "drop" ? { reason } : undefined);
      setAsking(null);
      refresh();
    } catch (e) {
      setActionError(handle(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-4 space-y-5">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <div className="text-xs text-zinc-500">
            task in{" "}
            <Link href={`/suites/${encodeURIComponent(data.suite.id)}`} className="text-zinc-300 hover:underline">{data.suite.name}</Link>
          </div>
          <h1 className="text-2xl font-semibold tracking-tight font-mono">{t.id}</h1>
          <div className="mt-1 text-sm text-zinc-400">
            {t.pr && prUrl(data.suite.repo, t.pr) ? (
              <a href={prUrl(data.suite.repo, t.pr) ?? undefined} target="_blank" rel="noopener noreferrer" className="hover:underline">PR #{t.pr}</a>
            ) : t.pr ? `PR #${t.pr}` : ""}
            {t.title ? `: ${t.title}` : ""}
          </div>
        </div>
        <div className="flex items-center gap-3">
          <StageBadge stage={t.stage} />
          {t.accepted && <span className="text-xs text-emerald-300">accepted</span>}
          {t.dropped && <span className="text-xs text-zinc-500">dropped{t.dropped_reason ? `: ${t.dropped_reason}` : ""}</span>}
        </div>
      </div>
      {error && <ErrorLine error={error} />}
      {t.scaffold && t.scaffold.warnings.length > 0 && (
        <ul className="list-disc pl-5 text-xs text-amber-300">{t.scaffold.warnings.map((w) => <li key={w}>{w}</li>)}</ul>
      )}
      {editable && (
        <div className="flex flex-wrap items-center gap-3">
          {!t.accepted && (
            <button type="button" onClick={() => setAsking("accept")} disabled={busy || asking !== null || t.stage !== "validated"} className="rounded-full border border-emerald-500/40 px-4 py-1 text-sm text-emerald-300 hover:bg-emerald-500/10 disabled:opacity-30">
              Accept
            </button>
          )}
          <button type="button" onClick={() => setAsking("drop")} disabled={busy || asking !== null || t.active_job !== null} title={t.active_job !== null ? "A job is working on this task; stop it or let it finish first" : undefined} className="rounded-full border border-white/15 px-4 py-1 text-sm text-zinc-300 hover:bg-white/5 disabled:opacity-30">
            Drop
          </button>
          {t.stage !== "validated" && !t.accepted && (
            <span className="text-xs text-zinc-500">Accept needs a PASS or WARN validation newer than the last authoring.</span>
          )}
          {actionError && <span className="text-xs text-rose-300">{actionError}</span>}
        </div>
      )}
      {editable && asking && (
        <ConfirmBar
          message={asking === "accept"
            ? `Accept ${task} into the eval suite? It is frozen with the suite later.`
            : `Drop ${task}? It leaves the eval suite; its folder stays in the suites home.`}
          confirmLabel={asking === "accept" ? "Accept" : "Drop"}
          withReason={asking === "drop"}
          busy={busy}
          onConfirm={(reason) => act(asking, reason)}
          onCancel={() => setAsking(null)}
        />
      )}
      <LatestJob task={t} jobs={data.jobs} frozen={data.frozen} />
      {editable && t.active_job === null && (
        <div className="space-y-3 rounded-xl border border-white/10 p-4">
          <div className="flex flex-wrap items-center gap-2">
            <button type="button" onClick={() => setBuild(build === "author" ? null : "author")} className={`rounded-full border px-4 py-1 text-sm ${build === "author" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              {t.author ? "Author again" : "Author"}
            </button>
            <button type="button" onClick={() => setBuild(build === "validate" ? null : "validate")} disabled={t.author?.status !== "ok"} title={t.author?.status !== "ok" ? "Validation needs a successful authoring session first" : undefined} className={`rounded-full border px-4 py-1 text-sm disabled:opacity-30 ${build === "validate" ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200" : "border-white/15 text-zinc-300 hover:bg-white/5"}`}>
              {t.validate ? "Validate again" : "Validate"}
            </button>
          </div>
          {build && (
            <BuildActions key={build} suite={suite} tasks={[task]} options={data.options} kind={build} defaults={lastAuthorChoice([t])} closeOnQueued onDone={() => { setBuild(null); refresh(); }} />
          )}
        </div>
      )}
      {t.active_job && (
        <div>
          <div className="mb-2 text-xs text-zinc-400">
            {t.active_job.kind === "author" ? "authoring session" : "validation"} job {t.active_job.id} is {t.active_job.status}
          </div>
          <LogTail runId={t.active_job.run_id} choices={[{ key: "console", label: "console.log", source: "console.log", trial: null }]} live />
        </div>
      )}
      {data.jobs.length > 0 && (
        <div className="text-xs text-zinc-400">
          jobs:{" "}
          {data.jobs.map((j) => (
            <Link key={j.id} href={`/runs/${encodeURIComponent(j.run_id)}`} className="mr-3 text-emerald-300 hover:underline">
              {j.kind} {j.id} ({j.status})
            </Link>
          ))}
        </div>
      )}
      <div className="flex flex-wrap gap-1 border-b border-white/10">
        {TABS.map((name) => (
          <button key={name} type="button" onClick={() => setTab(name)} className={`px-3 py-2 text-sm ${tab === name ? "border-b-2 border-emerald-400 text-white" : "text-zinc-400 hover:text-white"}`}>
            {name}
          </button>
        ))}
      </div>
      <div>
        {tab === "Issue" && <Text file={data.issue_md} empty="no issue.md" />}
        {tab === "Tests" && (
          <div className="space-y-4">
            {data.tests.length === 0 && (
              <div className="text-sm text-zinc-500">
                {t.author?.status === "ok" ? "The last authoring session left no files under tests/." : "No hidden tests yet: authoring writes them."}
              </div>
            )}
            {data.tests.map((f) => (
              <div key={f.path}>
                <div className="mb-1 font-mono text-xs text-zinc-400">{f.path}</div>
                {f.omitted ? <div className="text-xs text-amber-300">not shown: the page shows at most 512 KiB of tests</div> : <Text file={f as TextFile} empty="" />}
              </div>
            ))}
          </div>
        )}
        {tab === "Gold patch" && <Text file={data.gold_patch} empty="no gold_patch.diff" />}
        {tab === "Metadata" && <Json value={data.metadata} empty="no metadata" />}
        {tab === "Author" && (
          <div className="space-y-3">
            {data.author_notes && (
              <div>
                <div className="mb-1 text-xs text-zinc-400">author notes</div>
                <Text file={data.author_notes} empty="" />
              </div>
            )}
            <AuthorSummary task={t} />
            {data.author && <div className="text-xs text-zinc-500">raw record</div>}
            <Json value={data.author} empty="Not authored yet." />
          </div>
        )}
        {tab === "Validate" && (
          <div className="space-y-3">
            <ValidateSummary task={t} raw={data.validate} />
            {data.validate && <div className="text-xs text-zinc-500">raw record</div>}
            <Json value={data.validate} empty="Not validated yet." />
          </div>
        )}
      </div>
    </div>
  );
}
