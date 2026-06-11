import Link from "next/link";
import { API_BASE, getCalibration, getTask } from "@/lib/api";

export const dynamic = "force-dynamic";

interface Props {
  params: Promise<{ id: string }>;
}

function fmt(n: number | null | undefined): string {
  return n === null || n === undefined ? "—" : n.toFixed(2);
}

export default async function TaskDetail({ params }: Props) {
  const { id } = await params;
  const [task, calibration] = await Promise.all([getTask(id), getCalibration()]);

  if (!task) {
    return (
      <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
        <div className="max-w-4xl mx-auto">
          <Link href="/tasks" className="text-sm text-zinc-400 hover:text-white">← tasks</Link>
          <h1 className="text-3xl font-semibold tracking-tight mt-2 font-mono">{id}</h1>
          <p className="mt-6 text-sm text-zinc-400">Task not found. Is the backend running at {API_BASE}?</p>
        </div>
      </div>
    );
  }

  const m = task.metadata as Record<string, unknown>;
  const languages = (m.languages as string[]) ?? [];

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-4xl mx-auto">
        <Link href="/tasks" className="text-sm text-zinc-400 hover:text-white">← tasks</Link>
        <h1 className="text-3xl font-semibold tracking-tight mt-2 font-mono">{task.id}</h1>

        <div className="mt-4 flex flex-wrap gap-2 text-xs">
          {m.category ? <span className="px-2 py-1 rounded-full bg-white/10">{String(m.category)}</span> : null}
          {languages.map((l) => (
            <span key={l} className="px-2 py-1 rounded-full bg-sky-500/15 text-sky-300">{l}</span>
          ))}
          {m.difficulty ? <span className="px-2 py-1 rounded-full bg-white/10">{String(m.difficulty)}</span> : null}
          {m.source ? <span className="px-2 py-1 rounded-full bg-white/10">{String(m.source)}</span> : null}
        </div>

        {(() => {
          const calEntry = calibration?.tasks?.find((e) => e.task_id === id);
          if (!calEntry) return null;
          if (calEntry.status === "ok") {
            const disagrees = calEntry.agreement === false;
            return (
              <div className={`mt-4 text-sm border rounded-xl p-4 ${disagrees ? "border-amber-500/40 text-amber-300" : "border-white/10 text-zinc-400"}`}>
                <span className="font-medium text-white">Calibration:</span>{" "}
                {disagrees ? "empirical difficulty disagrees with label — " : ""}
                measured <span className="font-mono">{calEntry.empirical_difficulty}</span> (solve rate {calEntry.solve_rate?.toFixed(2)} ± {calEntry.solve_rate_stderr?.toFixed(2)}, {calEntry.n_attempts} attempts × {calEntry.n_models} models)
              </div>
            );
          }
          return (
            <div className="mt-4 text-sm border border-white/10 rounded-xl p-4 text-zinc-500">
              Not enough runs to calibrate ({calEntry.n_attempts} attempts from {calEntry.n_models} model{calEntry.n_models === 1 ? "" : "s"}).
            </div>
          );
        })()}

        <div className="mt-8 border border-white/10 rounded-xl p-6">
          <div className="text-zinc-400 mb-2 text-sm">Issue</div>
          <pre className="whitespace-pre-wrap text-sm text-zinc-200 font-sans">{task.issue}</pre>
        </div>

        <div className="mt-8">
          <div className="text-zinc-400 mb-3 text-sm">Runs ({task.runs.length})</div>
          {task.runs.length === 0 ? (
            <p className="text-sm text-zinc-500">
              No runs yet. Try <span className="font-mono">vulcanbench run --task {task.id} --model openai:gpt-4o</span>.
            </p>
          ) : (
            <div className="overflow-x-auto border border-white/10 rounded-xl">
              <table className="w-full text-sm">
                <thead className="bg-white/5 text-left">
                  <tr>
                    {["Run", "Model", "Total", "Functional", "Cost", "Time", ""].map((h, i) => (
                      <th key={i} className="p-4 font-normal text-zinc-400">{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-white/10">
                  {task.runs.map((r) => (
                    <tr key={r.run_id} className="hover:bg-white/5">
                      <td className="p-4 font-mono text-emerald-400">{r.run_id}</td>
                      <td className="p-4 text-zinc-400">{r.model ?? "?"}</td>
                      <td className="p-4 text-emerald-400 font-medium">{fmt(r.total)}</td>
                      <td className="p-4">{fmt(r.functional)}</td>
                      <td className="p-4 text-zinc-400">{r.cost_usd === null ? "—" : `$${r.cost_usd.toFixed(4)}`}</td>
                      <td className="p-4 text-zinc-400">{r.duration_s === null ? "—" : `${r.duration_s.toFixed(1)}s`}</td>
                      <td className="p-4"><Link href={`/run/${r.run_id}`} className="text-xs underline">trace</Link></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
