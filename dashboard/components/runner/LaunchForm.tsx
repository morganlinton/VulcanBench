"use client";

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

  return (
    <div className="mt-6">
      <SweepForm opts={opts} tag={tag} onTag={setTagChoice} refreshOptions={refresh} />
      <div className="mt-6">
        <TagCard onCut={(t) => { if (t) setTagChoice(t); refresh(); }} />
      </div>
    </div>
  );
}
