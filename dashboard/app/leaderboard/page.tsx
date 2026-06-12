import Link from "next/link";
import { getEffortSensitivity, getLeaderboard, getModelLeaderboard } from "@/lib/api";

export const dynamic = "force-dynamic";

const API_LABEL = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

function fmt(n: number | null | undefined): string {
  return n === null || n === undefined ? "—" : n.toFixed(2);
}

function money(n: number | null | undefined, known: boolean): string {
  if (!known) return "?";
  return n === null || n === undefined ? "—" : `$${n.toFixed(4)}`;
}

function pct(n: number | null | undefined): string {
  return n === null || n === undefined ? "—" : `${(n * 100).toFixed(0)}%`;
}

interface Props {
  searchParams: Promise<{ by?: string; effort?: string; task_complexity?: string }>;
}

const EMPTY = (
  <div className="border border-white/10 rounded-xl p-8 text-sm text-zinc-400">
    No runs found. Start the backend and record a run:
    <pre className="mt-3 bg-black/50 p-3 rounded-lg text-emerald-400 overflow-x-auto">
{`uvicorn backend.app:app --port 8000   # serves ./runs
vulcanbench run --suite v1 --model mock:synthetic`}
    </pre>
  </div>
);

export default async function Leaderboard({ searchParams }: Props) {
  const { by, effort, task_complexity } = await searchParams;
  const view = by === "run" ? "run" : "model";
  const effortFilter = effort || undefined;
  const complexityFilter = task_complexity || undefined;

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-6xl mx-auto">
        <div className="flex justify-between items-baseline mb-2">
          <h1 className="text-4xl font-semibold tracking-tight">Leaderboard</h1>
          <Link href="/" className="text-sm text-zinc-400 hover:text-white">← home</Link>
        </div>
        <div className="flex gap-4 mb-8 text-sm">
          <Link href="/leaderboard" className={view === "model" ? "text-emerald-400" : "text-zinc-500 hover:text-white"}>by model</Link>
          <Link href="/leaderboard?by=run" className={view === "run" ? "text-emerald-400" : "text-zinc-500 hover:text-white"}>by run</Link>
        </div>

        <div className="mb-6 flex flex-wrap items-center gap-2 text-xs">
          {["all", "low", "medium", "high", "extra-high"].map((level) => {
            const href = level === "all" ? `/leaderboard?by=${view}` : `/leaderboard?by=${view}&effort=${level}`;
            const active = (level === "all" && !effortFilter) || effortFilter === level;
            return (
              <Link
                key={level}
                href={href}
                className={active ? "rounded-md bg-emerald-500 px-3 py-2 text-black" : "rounded-md border border-white/10 px-3 py-2 text-zinc-400 hover:text-white"}
              >
                {level}
              </Link>
            );
          })}
        </div>

        {view === "model" ? (
          <ModelTable effort={effortFilter} taskComplexity={complexityFilter} />
        ) : (
          <RunTable effort={effortFilter} taskComplexity={complexityFilter} />
        )}

        <EffortSensitivityTable />

        <p className="mt-4 text-xs text-zinc-500">Live data from the backend ({API_LABEL}).</p>
      </div>
    </div>
  );
}

async function ModelTable({
  effort,
  taskComplexity,
}: {
  effort?: string;
  taskComplexity?: string;
}) {
  const rows = await getModelLeaderboard(undefined, effort, taskComplexity);
  if (rows.length === 0) return EMPTY;
  return (
    <div className="overflow-x-auto border border-white/10 rounded-xl">
      <table className="w-full text-sm">
        <thead className="bg-white/5 text-left">
          <tr>
            {["Model", "Tasks", "Runs", "pass@1", "pass@k", "Avg total", "Cost", "Avg time"].map((h) => (
              <th key={h} className="p-4 font-normal text-zinc-400">{h}</th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-white/10">
          {rows.map((a) => (
            <tr key={a.model} className="hover:bg-white/5">
              <td className="p-4 font-mono text-emerald-400">{a.model}</td>
              <td className="p-4">{a.n_tasks}</td>
              <td className="p-4 text-zinc-400">{a.n_runs}</td>
              <td className="p-4 text-emerald-400 font-medium">{pct(a.pass_at_1)} <span className="text-zinc-500">± {pct(a.pass_at_1_stderr)}</span></td>
              <td className="p-4 text-zinc-400">{pct(a.pass_at_k)}</td>
              <td className="p-4">{fmt(a.avg_total)}</td>
              <td className="p-4 text-zinc-400">{money(a.total_cost, a.cost_known)}</td>
              <td className="p-4 text-zinc-400">{a.avg_duration_s === null ? "—" : `${a.avg_duration_s.toFixed(1)}s`}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

async function RunTable({
  effort,
  taskComplexity,
}: {
  effort?: string;
  taskComplexity?: string;
}) {
  const rows = await getLeaderboard(undefined, effort, taskComplexity);
  if (rows.length === 0) return EMPTY;
  return (
    <div className="overflow-x-auto border border-white/10 rounded-xl">
      <table className="w-full text-sm">
        <thead className="bg-white/5 text-left">
          <tr>
            {["Run", "Task", "Model", "Effort", "Complexity", "Total", "Functional", "Cost", "Time", ""].map((h, i) => (
              <th key={i} className="p-4 font-normal text-zinc-400">{h}</th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-white/10">
          {rows.map((r) => (
            <tr key={r.run_id} className="hover:bg-white/5">
              <td className="p-4 font-mono text-emerald-400">{r.run_id}</td>
              <td className="p-4">{r.task_id ?? "?"}</td>
              <td className="p-4 text-zinc-400">{r.model ?? "?"}</td>
              <td className="p-4 text-zinc-400">{r.effort_requested ?? "—"}</td>
              <td className="p-4 text-zinc-400">{r.task_complexity ?? "—"}</td>
              <td className="p-4 text-emerald-400 font-medium">{fmt(r.total)}</td>
              <td className="p-4">{fmt(r.functional)}</td>
              <td className="p-4 text-zinc-400">{r.cost_usd === null ? "—" : `$${r.cost_usd.toFixed(4)}`}</td>
              <td className="p-4 text-zinc-400">{r.duration_s === null ? "—" : `${r.duration_s.toFixed(1)}s`}</td>
              <td className="p-4">
                <Link href={`/run/${r.run_id}`} className="text-xs underline">view trace</Link>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

async function EffortSensitivityTable() {
  const data = await getEffortSensitivity();
  if (!data.available || data.strata.length === 0) return null;
  return (
    <div className="mt-10">
      <h2 className="mb-3 text-xl font-semibold tracking-tight">Effort Sensitivity</h2>
      {data.warnings.length > 0 && (
        <div className="mb-4 space-y-2 text-xs text-amber-300">
          {data.warnings.map((warning) => (
            <div key={warning}>{warning}</div>
          ))}
        </div>
      )}
      <div className="overflow-x-auto border border-white/10 rounded-lg">
        <table className="w-full text-sm">
          <thead className="bg-white/5 text-left">
            <tr>
              {["Model", "Language", "Scale", "Difficulty", "Complexity", "low pass@1", "high pass@1", "Delta", "Cost ratio", "Latency ratio", "Status"].map((h) => (
                <th key={h} className="p-4 font-normal text-zinc-400">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-white/10">
            {data.strata.map((row) => (
              <tr key={`${row.model}-${row.language}-${row.repo_scale}-${row.difficulty}-${row.task_complexity}`} className="hover:bg-white/5">
                <td className="p-4 font-mono text-emerald-400">{row.model}</td>
                <td className="p-4">{row.language}</td>
                <td className="p-4 text-zinc-400">{row.repo_scale}</td>
                <td className="p-4 text-zinc-400">{row.difficulty}</td>
                <td className="p-4 text-zinc-400">{row.task_complexity}</td>
                <td className="p-4">{pct(row.efforts.low?.pass_at_1)}</td>
                <td className="p-4">{pct(row.efforts.high?.pass_at_1)}</td>
                <td className="p-4">{pct(row.high_minus_low_pass_at_1)}</td>
                <td className="p-4 text-zinc-400">{fmt(row.high_cost_ratio)}</td>
                <td className="p-4 text-zinc-400">{fmt(row.high_latency_ratio)}</td>
                <td className="p-4">{row.classification}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
