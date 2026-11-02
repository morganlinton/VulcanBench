import Link from "next/link";
import { Suspense } from "react";
import { LaunchForm } from "@/components/runner/LaunchForm";

export const dynamic = "force-dynamic";

export default function LaunchPage() {
  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-5xl mx-auto">
        <div className="flex justify-between items-baseline mb-2">
          <h1 className="text-4xl font-semibold tracking-tight">Launch</h1>
          <div className="flex gap-4 text-sm">
            <Link href="/runs" className="text-zinc-400 hover:text-white">runs</Link>
            <Link href="/" className="text-zinc-400 hover:text-white">← home</Link>
          </div>
        </div>
        <p className="text-sm text-zinc-400 max-w-3xl">
          Benchmark a model on VulcanBench Frontier v4 or any other frozen suite, across effort levels.
          Build the plan, read the exact commands and the estimate, then confirm. Jobs go to a queue that
          runs one task at a time; nothing starts without the confirm click and nothing retries on its own.
        </p>
        <Suspense fallback={<div className="mt-6 text-sm text-zinc-500">loading</div>}>
          <LaunchForm />
        </Suspense>
      </div>
    </div>
  );
}
