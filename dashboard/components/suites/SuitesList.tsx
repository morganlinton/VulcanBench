"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { button, field, label } from "@/components/runner/launchShared";
import { usePolling } from "@/components/runner/usePolling";
import { runnerGet, runnerPost } from "@/lib/runner";
import {
  REPO_RE,
  normalizeRepo,
  SUITE_ID_RE,
  SUITES_API,
  STAGE_LABEL,
  slugify,
  type Stage,
  type SuiteRow,
  type SuitesList as SuitesListData,
} from "@/lib/suites";
import { ErrorLine, useGate } from "./BuilderGate";

const STAGE_CLASS: Record<Stage, string> = {
  missing: "border-rose-500/40 text-rose-300",
  scaffolded: "border-white/15 text-zinc-300",
  authoring: "border-emerald-500/40 text-emerald-300",
  authored: "border-sky-500/40 text-sky-300",
  author_failed: "border-rose-500/40 text-rose-300",
  validating: "border-emerald-500/40 text-emerald-300",
  validated: "border-emerald-500/40 text-emerald-200",
  validation_failed: "border-rose-500/40 text-rose-300",
};

export function StageBadge({ stage }: { stage: Stage }) {
  const active = stage === "authoring" || stage === "validating";
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs ${STAGE_CLASS[stage]}`}>
      {active && <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />}
      {STAGE_LABEL[stage]}
    </span>
  );
}

export function repoName(r: SuiteRow["repo"]): string {
  return r ? `${r.owner}/${r.name}` : "?";
}

export function SuitesList() {
  const { data, error } = usePolling<SuitesListData>(() => runnerGet<SuitesListData>(SUITES_API), 5000, "suites");
  if (error && !data) return <ErrorLine error={error} />;
  if (!data) return <div className="mt-6 text-sm text-zinc-500">loading eval suites</div>;
  return (
    <div className="mt-6 space-y-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div className="text-xs text-zinc-500">
          suites home <span className="font-mono text-zinc-300">{data.suites_home}</span>
        </div>
        <Link href="/suites/new" className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>New eval suite</Link>
      </div>
      {data.suites.length === 0 ? (
        <div className="rounded-xl border border-white/10 p-6 text-sm text-zinc-400">
          No eval suites yet. Start one from a GitHub repository you can read with this Mac&apos;s <code className="font-mono">gh</code> login.
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead className="text-left text-xs uppercase tracking-wider text-zinc-500">
              <tr>
                <th className="px-3 py-2">Eval suite</th>
                <th className="px-3 py-2">Repository</th>
                <th className="px-3 py-2">Status</th>
                <th className="px-3 py-2">Tasks</th>
                <th className="px-3 py-2">Stages</th>
              </tr>
            </thead>
            <tbody>
              {data.suites.map((s) => (
                <tr key={s.id} className="border-t border-white/5 hover:bg-white/[0.02]">
                  <td className="px-3 py-2">
                    <Link href={`/suites/${encodeURIComponent(s.id)}`} className="text-emerald-300 hover:underline">{s.name}</Link>
                    <div className="font-mono text-xs text-zinc-500">{s.id}</div>
                  </td>
                  <td className="px-3 py-2 font-mono text-xs">{repoName(s.repo)}</td>
                  <td className="px-3 py-2 text-xs">
                    {s.status === "frozen" ? (
                      <span className="text-zinc-200">frozen {s.version} {s.frozen_at}</span>
                    ) : (
                      <span className="text-amber-300">draft</span>
                    )}
                    {s.active_job && <div className="text-emerald-300">job {s.active_job.id} {s.active_job.status}</div>}
                  </td>
                  <td className="px-3 py-2 text-xs">{s.n_accepted} accepted of {s.n_tasks}{s.n_dropped ? `, ${s.n_dropped} dropped` : ""}</td>
                  <td className="px-3 py-2">
                    <div className="flex flex-wrap gap-1">
                      {(Object.entries(s.stages) as [Stage, number][]).map(([st, n]) => (
                        <span key={st} className="text-xs text-zinc-400">{n} {STAGE_LABEL[st]}</span>
                      ))}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

export function NewSuiteForm() {
  const router = useRouter();
  const { handle } = useGate();
  const [name, setName] = useState("");
  const [id, setId] = useState("");
  const [idEdited, setIdEdited] = useState(false);
  const [repo, setRepo] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const suiteId = idEdited ? id : slugify(name);
  const repoId = normalizeRepo(repo);
  const repoOk = REPO_RE.test(repoId);
  const ok = name.trim() && SUITE_ID_RE.test(suiteId) && repoOk;
  const missing = !name.trim() ? "a name" : !SUITE_ID_RE.test(suiteId) ? "a valid id" : !repoOk ? "a repository" : null;
  const create = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const row = await runnerPost<SuiteRow>(SUITES_API, { id: suiteId, display_name: name.trim(), repo: repoId });
      router.push(`/suites/${encodeURIComponent(row.id)}`);
    } catch (e) {
      setError(handle(e));
      setBusy(false);
    }
  };
  return (
    <form onSubmit={create} className="mt-6 max-w-xl space-y-4 rounded-2xl border border-white/10 p-5">
      <div>
        <label className={label} htmlFor="suite-name">Name</label>
        <input id="suite-name" className={field} value={name} onChange={(e) => setName(e.target.value)} placeholder="Acme API regressions" maxLength={120} />
      </div>
      <div>
        <label className={label} htmlFor="suite-id">Id</label>
        <input
          id="suite-id"
          className={`${field} font-mono`}
          value={suiteId}
          onChange={(e) => { setIdEdited(true); setId(e.target.value); }}
          maxLength={64}
        />
        <div className="mt-1 text-xs text-zinc-500">
          Lower-case letters, digits and hyphens, 3 to 64 characters. It becomes the folder in the suites home and
          the name you pick on the Launch page.
        </div>
        {suiteId && !SUITE_ID_RE.test(suiteId) && <div className="mt-1 text-xs text-rose-300">not a valid id</div>}
      </div>
      <div>
        <label className={label} htmlFor="suite-repo">GitHub repository</label>
        <input id="suite-repo" className={`${field} font-mono`} value={repo} onChange={(e) => setRepo(e.target.value)} placeholder="owner/name or a github.com link" />
        <div className="mt-1 text-xs text-zinc-500">
          Pull requests are read with this Mac&apos;s <code className="font-mono">gh</code> login, so a private repository
          works if you can read it. Its code stays in the suites home and is never committed to VulcanBench.
        </div>
        {repo.trim() && !repoOk && <div className="mt-1 text-xs text-rose-300">use owner/name, for example acme/api</div>}
        {repo.trim() && repoOk && repoId !== repo.trim() && <div className="mt-1 font-mono text-xs text-zinc-400">reads {repoId}</div>}
      </div>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      <button type="submit" disabled={!ok || busy} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
        {busy ? "Creating" : "Create eval suite"}
      </button>
      {missing && <span className="ml-3 text-xs text-zinc-500">needs {missing}</span>}
    </form>
  );
}
