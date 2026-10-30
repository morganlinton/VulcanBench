import Link from "next/link";
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
          Plan a run, read the exact command and the estimate, check usage, then confirm. The job goes
          to a queue that runs one task at a time; nothing starts without the confirm click and nothing
          retries on its own.
        </p>
        <LaunchForm />
      </div>
    </div>
  );
}
