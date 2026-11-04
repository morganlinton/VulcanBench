"use client";

import Link from "next/link";
import { useState } from "react";
import { usePolling } from "@/components/runner/usePolling";
import { runnerGet, runnerPost } from "@/lib/runner";
import { taskPath, type TaskDetail, type TaskRow, type TextFile } from "@/lib/suites";
import { ErrorLine, useGate } from "./BuilderGate";
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

export function TaskReview({ suite, task }: { suite: string; task: string }) {
  const { handle } = useGate();
  const [tab, setTab] = useState<Tab>("Issue");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const { data, error, refresh } = usePolling<TaskDetail>(
    () => runnerGet<TaskDetail>(taskPath(suite, task)),
    5000,
    `${suite}/${task}`,
  );
  if (error && !data) return <ErrorLine error={error} />;
  if (!data) return <div className="mt-6 text-sm text-zinc-500">loading task</div>;
  const t = data.task;
  const editable = !data.frozen && !t.dropped;

  const act = async (what: "accept" | "drop") => {
    let body: unknown = undefined;
    if (what === "accept" && !window.confirm(`Accept ${task} into the suite? It is frozen with the suite later.`)) return;
    if (what === "drop") {
      const reason = window.prompt(`Drop ${task}? Its folder stays in the suites home. Reason (optional):`, "");
      if (reason === null) return;
      body = { reason };
    }
    setBusy(true);
    setActionError(null);
    try {
      await runnerPost<TaskRow>(`${taskPath(suite, task)}/${what}`, body);
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
          <h1 className="text-2xl font-semibold tracking-tight font-mono">{t.id}</h1>
          <div className="mt-1 text-sm text-zinc-400">
            {t.pr ? `PR #${t.pr}` : ""}{t.title ? `: ${t.title}` : ""}
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
            <button type="button" onClick={() => act("accept")} disabled={busy || t.stage !== "validated"} className="rounded-full border border-emerald-500/40 px-4 py-1 text-sm text-emerald-300 hover:bg-emerald-500/10 disabled:opacity-30">
              Accept
            </button>
          )}
          <button type="button" onClick={() => act("drop")} disabled={busy || t.active_job !== null} className="rounded-full border border-white/15 px-4 py-1 text-sm text-zinc-300 hover:bg-white/5 disabled:opacity-30">
            Drop
          </button>
          {t.stage !== "validated" && !t.accepted && (
            <span className="text-xs text-zinc-500">Accept needs a PASS or WARN validation newer than the last authoring.</span>
          )}
          {actionError && <span className="text-xs text-rose-300">{actionError}</span>}
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
            {data.tests.length === 0 && <div className="text-sm text-zinc-500">No hidden tests yet: authoring writes them.</div>}
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
            <Json value={data.author} empty="Not authored yet." />
          </div>
        )}
        {tab === "Validate" && <Json value={data.validate} empty="Not validated yet." />}
      </div>
    </div>
  );
}
