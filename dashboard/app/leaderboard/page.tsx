import Link from "next/link";
import { getLeaderboard, getModelLeaderboard } from "@/lib/api";

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
  searchParams: Promise<{ by?: string }>;
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
  const { by } = await searchParams;
  const view = by === "run" ? "run" : "model";

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

        {view === "model" ? <ModelTable /> : <RunTable />}

        <p className="mt-4 text-xs text-zinc-500">Live data from the backend ({API_LABEL}).</p>
      </div>
    </div>
  );
}

async function ModelTable() {
  const rows = await getModelLeaderboard();
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

async function RunTable() {
  const rows = await getLeaderboard();
  if (rows.length === 0) return EMPTY;
  return (
    <div className="overflow-x-auto border border-white/10 rounded-xl">
      <table className="w-full text-sm">
        <thead className="bg-white/5 text-left">
          <tr>
            {["Run", "Task", "Model", "Total", "Functional", "Cost", "Time", ""].map((h, i) => (
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
