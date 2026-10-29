import Link from "next/link";
import { RunsTable } from "@/components/runner/RunsTable";

export const dynamic = "force-dynamic";

export default function RunsPage() {
  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-7xl mx-auto">
        <div className="flex justify-between items-baseline mb-2">
          <h1 className="text-4xl font-semibold tracking-tight">Runs</h1>
          <Link href="/" className="text-sm text-zinc-400 hover:text-white">← home</Link>
        </div>
        <p className="text-sm text-zinc-400 max-w-3xl">
          Every gate job and harness run under the configured roots, running or finished, whether
          the GUI launched it or not. Reads only: nothing on this page starts or stops anything.
        </p>
        <RunsTable />
      </div>
    </div>
  );
}
