"use client";

import { useEffect, useRef, useState } from "react";
import { fmtBytes, logPath, runnerGet, type LogTail as LogTailData } from "@/lib/runner";
import { usePolling } from "./usePolling";

export interface LogChoice {
  key: string;
  label: string;
  source: string;
  trial: string | null;
}

export function LogTail({
  runId,
  choices,
  live,
}: {
  runId: string;
  choices: LogChoice[];
  live: boolean;
}) {
  const [key, setKey] = useState<string | null>(null);
  const [follow, setFollow] = useState(true);
  const preRef = useRef<HTMLPreElement>(null);
  // A choice the user picked wins while it still exists; otherwise the first one.
  const choice = choices.find((c) => c.key === key) ?? choices[0];

  const { data, error } = usePolling<LogTailData | null>(
    () => (choice ? runnerGet<LogTailData>(logPath(runId, choice.source, choice.trial)) : Promise.resolve(null)),
    live ? 3000 : 0,
    `${runId}|${choice?.key ?? ""}`,
  );

  useEffect(() => {
    if (follow && preRef.current) preRef.current.scrollTop = preRef.current.scrollHeight;
  }, [data, follow]);

  if (choices.length === 0) {
    return <div className="text-sm text-zinc-500">No log files on disk for this run yet.</div>;
  }

  return (
    <div>
      <div className="flex flex-wrap items-center gap-3 text-xs">
        <select
          value={choice?.key ?? ""}
          onChange={(e) => setKey(e.target.value)}
          className="bg-zinc-900 border border-white/10 rounded-md px-2 py-1 text-zinc-200"
        >
          {choices.map((c) => (
            <option key={c.key} value={c.key}>{c.label}</option>
          ))}
        </select>
        <label className="flex items-center gap-1 text-zinc-400">
          <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
          follow
        </label>
        {data && (
          <span className="text-zinc-500">
            {fmtBytes(data.size)}{data.truncated ? ", showing the tail" : ""}, redacted
          </span>
        )}
        {live && <span className="text-zinc-600">refreshes every 3 s</span>}
        {error && <span className="text-amber-300">{error.message}</span>}
      </div>
      <pre
        ref={preRef}
        className="mt-2 h-[28rem] overflow-auto rounded-xl border border-white/10 bg-black p-4 text-xs leading-5 text-zinc-300 font-mono whitespace-pre-wrap break-all"
      >
        {data?.text ?? (error ? "" : "loading")}
      </pre>
    </div>
  );
}
