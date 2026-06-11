import Link from "next/link";
import { getTasks } from "@/lib/api";

export const dynamic = "force-dynamic";

export default async function Tasks() {
  const tasks = await getTasks();

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-5xl mx-auto">
        <div className="flex justify-between items-baseline mb-8">
          <h1 className="text-4xl font-semibold tracking-tight">Tasks</h1>
          <div className="flex gap-4 text-sm">
            <Link href="/submit" className="text-emerald-400 hover:text-emerald-300">+ submit a task</Link>
            <Link href="/" className="text-zinc-400 hover:text-white">← home</Link>
          </div>
        </div>

        {tasks.length === 0 ? (
          <div className="border border-white/10 rounded-xl p-8 text-sm text-zinc-400">
            No tasks found. Start the backend (<span className="font-mono">uvicorn backend.app:app --port 8000</span>).
          </div>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2">
            {tasks.map((t) => (
              <Link
                key={t.id}
                href={`/task/${t.id}`}
                className="block border border-white/10 rounded-xl p-5 hover:bg-white/5"
              >
                <div className="font-mono text-emerald-400">{t.id}</div>
                <div className="mt-3 flex flex-wrap gap-2 text-xs">
                  {t.category ? <span className="px-2 py-1 rounded-full bg-white/10">{t.category}</span> : null}
                  {t.languages.map((l) => (
                    <span key={l} className="px-2 py-1 rounded-full bg-sky-500/15 text-sky-300">{l}</span>
                  ))}
                  {t.difficulty ? <span className="px-2 py-1 rounded-full bg-white/10">{t.difficulty}</span> : null}
                  {t.empirical_difficulty ? (
                    <span className={`px-2 py-1 rounded-full ${t.calibration_status === "ok" && t.difficulty && t.empirical_difficulty !== (t.difficulty === "trivial" ? "easy" : t.difficulty) ? "bg-amber-500/20 text-amber-300" : "bg-white/10"}`}>
                      measured: {t.empirical_difficulty}
                    </span>
                  ) : null}
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
