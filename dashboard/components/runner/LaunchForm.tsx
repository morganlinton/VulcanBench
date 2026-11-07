"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useState } from "react";
import { runnerGet, type LaunchOptions } from "@/lib/runner";
import { RunnerUnavailable } from "./RunnerUnavailable";
import { SweepForm } from "./SweepForm";
import { TagCard } from "./TagCard";
import { usePolling } from "./usePolling";

export function LaunchForm() {
  const params = useSearchParams();
  const [tagChoice, setTagChoice] = useState<string | null>(params.get("tag"));
  const { data: opts, error, refresh } = usePolling<LaunchOptions>(
    () => runnerGet<LaunchOptions>(`/api/runner/options${tagChoice ? `?tag=${encodeURIComponent(tagChoice)}` : ""}`),
    0,
    tagChoice ?? "",
  );

  if (error && !opts) return <RunnerUnavailable error={error} />;
  if (!opts) return <div className="mt-6 text-sm text-zinc-500">loading options</div>;
  const tag = opts.tag ?? "";

  const custom = opts.suites.filter((s) => s.custom);
  return (
    <div className="mt-6">
      <div className="mb-6 grid gap-3 sm:grid-cols-2">
        <div className="rounded-xl border border-emerald-500/40 bg-emerald-500/[0.04] p-4">
          <div className="font-medium">Benchmark a model</div>
          <div className="mt-1 text-xs text-zinc-400">
            Run a frozen eval suite (VulcanBench Frontier v4 or one you built) for one model across effort levels. The form is below.
          </div>
        </div>
        <Link href="/suites" className="rounded-xl border border-white/10 p-4 hover:border-emerald-500/40 hover:bg-white/[0.02]">
          <div className="font-medium">Build your own eval suite</div>
          <div className="mt-1 text-xs text-zinc-400">
            Turn merged pull requests from your own GitHub repository into benchmark tasks, for subscribers.
            {custom.length > 0 ? ` ${custom.length} of your own frozen.` : ""}
          </div>
        </Link>
      </div>
      <SweepForm opts={opts} tag={tag} onTag={setTagChoice} refreshOptions={refresh} />
      <div className="mt-6">
        <TagCard onCut={(t) => { if (t) setTagChoice(t); refresh(); }} />
      </div>
    </div>
  );
}
