import Link from "next/link";
import { Suspense } from "react";
import { BuilderGate } from "./BuilderGate";

// The frame every Suites page shares: header links, then the unlock gate.
export function SuitesShell({ back, children }: { back?: { href: string; label: string }; children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-6xl mx-auto">
        <div className="flex justify-between items-baseline">
          {back ? (
            <Link href={back.href} className="text-sm text-zinc-400 hover:text-white">← {back.label}</Link>
          ) : (
            <h1 className="text-4xl font-semibold tracking-tight">Suites</h1>
          )}
          <div className="flex gap-4 text-sm">
            <Link href="/launch" className="text-zinc-400 hover:text-white">launch</Link>
            <Link href="/runs" className="text-zinc-400 hover:text-white">runs</Link>
            <Link href="/" className="text-zinc-400 hover:text-white">home</Link>
          </div>
        </div>
        <Suspense fallback={<div className="mt-6 text-sm text-zinc-500">loading</div>}>
          <BuilderGate>{children}</BuilderGate>
        </Suspense>
      </div>
    </div>
  );
}
