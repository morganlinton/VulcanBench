import Link from "next/link";
import { API_BASE, getRun, getRunPatch, getTrace } from "@/lib/api";

export const dynamic = "force-dynamic";

interface Props {
  params: Promise<{ id: string }>;
}

const SCORE_KEYS = ["functional", "efficiency", "quality", "security", "human_like", "total"];

function DiffView({ patch }: { patch: string }) {
  const lines = patch.split("\n");
  return (
    <pre className="text-xs font-mono overflow-x-auto leading-5">
      {lines.map((ln, i) => {
        let cls = "text-zinc-400";
        if (ln.startsWith("+") && !ln.startsWith("+++")) cls = "text-emerald-400 bg-emerald-500/10";
        else if (ln.startsWith("-") && !ln.startsWith("---")) cls = "text-rose-400 bg-rose-500/10";
        else if (ln.startsWith("@@")) cls = "text-sky-400";
        else if (ln.startsWith("diff ") || ln.startsWith("+++") || ln.startsWith("---")) cls = "text-zinc-500";
        return <div key={i} className={cls}>{ln || " "}</div>;
      })}
    </pre>
  );
}

export default async function RunDetail({ params }: Props) {
  const { id } = await params;
  const [summary, trace, patch] = await Promise.all([getRun(id), getTrace(id), getRunPatch(id)]);

  if (!summary) {
    return (
      <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
        <div className="max-w-4xl mx-auto">
          <Link href="/leaderboard" className="text-sm text-zinc-400 hover:text-white">← leaderboard</Link>
          <h1 className="text-3xl font-semibold tracking-tight mt-2">Run {id}</h1>
          <p className="mt-6 text-sm text-zinc-400">
            Run not found. Is the backend running at {API_BASE}?
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-4xl mx-auto">
        <Link href="/leaderboard" className="text-sm text-zinc-400 hover:text-white">← leaderboard</Link>
        <h1 className="text-3xl font-semibold tracking-tight mt-2 font-mono">{summary.run_id}</h1>

        <div className="mt-8 grid grid-cols-2 gap-4 text-sm">
          <div className="border border-white/10 rounded-xl p-6">
            <div className="text-zinc-400 mb-3">Summary</div>
            <div>task: <Link href={`/task/${summary.task_id}`} className="text-emerald-400 hover:underline">{summary.task_id}</Link></div>
            <div>model: <span className="text-zinc-300">{summary.model}</span></div>
            <div>steps: <span className="text-zinc-300">{summary.steps}</span></div>
            {summary.total_tokens !== undefined && (
              <div>tokens: <span className="text-zinc-300">{summary.total_tokens}</span></div>
            )}
            <a
              href={`${API_BASE}/runs/${summary.run_id}/replay.html`}
              className="mt-4 inline-block underline text-emerald-400"
              target="_blank"
              rel="noreferrer"
            >
              open replay.html →
            </a>
          </div>

          <div className="border border-white/10 rounded-xl p-6">
            <div className="text-zinc-400 mb-3">Scores</div>
            <div className="space-y-1 font-mono">
              {SCORE_KEYS.map((k) => {
                const v = summary.scores?.[k];
                return (
                  <div key={k} className="flex justify-between">
                    <span className="text-zinc-400">{k}</span>
                    <span className={v === null || v === undefined ? "text-zinc-600" : "text-emerald-400"}>
                      {v === null || v === undefined ? "n/a" : v.toFixed(4)}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <div className="mt-6 border border-white/10 rounded-xl p-6">
          <div className="text-zinc-400 mb-3">Timeline ({trace.length} events)</div>
          <div className="space-y-1 text-xs font-mono">
            {trace.map((ev) => (
              <div key={ev.step} className="flex gap-3">
                <span className="text-zinc-600 w-6 text-right">{ev.step}</span>
                <span className="text-sky-400 w-40">{ev.type}</span>
                <span className="text-zinc-500 truncate">{summarizeEvent(ev.type, ev.data)}</span>
              </div>
            ))}
            {trace.length === 0 && (
              <div className="text-zinc-500">No trace events available.</div>
            )}
          </div>
        </div>

        <div className="mt-6 border border-white/10 rounded-xl p-6">
          <div className="text-zinc-400 mb-3">Patch (final.patch)</div>
          {patch.trim() ? (
            <DiffView patch={patch} />
          ) : (
            <div className="text-xs text-zinc-500">No diff captured for this run.</div>
          )}
        </div>

        {summary.replay_command && (
          <p className="mt-4 text-xs text-zinc-500">
            Reproduce: <span className="font-mono text-zinc-400">{summary.replay_command}</span>
          </p>
        )}
      </div>
    </div>
  );
}

function summarizeEvent(type: string, data: Record<string, unknown>): string {
  if (type === "tool_call") return `${data.tool ?? ""} ${JSON.stringify(data.args ?? {})}`;
  if (type === "tool_observation") return data.error ? `error: ${data.error}` : "ok";
  if (type === "llm_response") {
    const content = typeof data.content === "string" ? data.content : "";
    return content.slice(0, 80);
  }
  if (type === "metric_computed") return JSON.stringify(data);
  return "";
}
