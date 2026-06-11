import Link from "next/link";
import { REPO } from "@/lib/api";

export default function VulcanBenchHome() {
  return (
    <div className="min-h-screen bg-black text-white font-sans">
      <div className="max-w-5xl mx-auto px-8 py-20">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-3 h-3 bg-emerald-500 rounded-full animate-pulse" />
          <span className="uppercase tracking-[3px] text-xs text-emerald-500">v1 mvp</span>
        </div>
        <h1 className="text-7xl font-semibold tracking-tighter">VulcanBench</h1>
        <p className="mt-4 max-w-md text-xl text-zinc-400">
          Reproducible, transparent LLM benchmarking for real software engineering tasks.
        </p>
        <div className="mt-10 flex flex-wrap gap-4">
          <Link href="/leaderboard" className="inline-flex h-12 items-center rounded-full bg-white px-8 text-sm font-medium text-black hover:bg-zinc-200">Leaderboard</Link>
          <Link href="/tasks" className="inline-flex h-12 items-center rounded-full border border-white/20 px-8 text-sm hover:bg-white/5">Tasks</Link>
          <Link href="/submit" className="inline-flex h-12 items-center rounded-full border border-white/20 px-8 text-sm hover:bg-white/5">Submit a task</Link>
          <a href={`https://github.com/${REPO}`} className="inline-flex h-12 items-center rounded-full border border-white/20 px-8 text-sm hover:bg-white/5">GitHub</a>
        </div>
        <div className="mt-16 text-xs text-zinc-500">
          Multi-file tasks across Python, Go &amp; TypeScript • five-metric scoring
          (functional, quality, security, efficiency, human-like) • per-model
          leaderboard with cost &amp; latency • Docker-sandboxed, reproducible runs.
        </div>
      </div>
    </div>
  );
}
