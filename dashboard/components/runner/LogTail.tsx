"use client";

import { useEffect, useRef, useState } from "react";
import { fmtBytes, logPath, logStreamPath, runnerGet, type LogTail as LogTailData } from "@/lib/runner";

export interface LogChoice {
  key: string;
  label: string;
  source: string;
  trial: string | null;
}

const MAX_BUFFER = 400_000;

// A live run streams its log as server-sent events (the tail once, then only
// what was appended); a finished run fetches the tail once. If the stream
// cannot be opened the viewer falls back to polling every 3 s.
function Viewer({ runId, source, trial, live, follow }: { runId: string; source: string; trial: string | null; live: boolean; follow: boolean }) {
  const [text, setText] = useState("");
  const [size, setSize] = useState<number | null>(null);
  const [truncated, setTruncated] = useState(false);
  const [fallback, setFallback] = useState(false);
  const [generation, setGeneration] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const preRef = useRef<HTMLPreElement>(null);
  const mode: "stream" | "poll" | "once" = live ? (fallback ? "poll" : "stream") : "once";

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let es: EventSource | null = null;

    const fetchOnce = async () => {
      try {
        const d = await runnerGet<LogTailData>(logPath(runId, source, trial));
        if (cancelled) return;
        setText(d.text);
        setSize(d.size);
        setTruncated(d.truncated);
        setError(null);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled && mode === "poll") timer = setTimeout(fetchOnce, 3000);
      }
    };

    if (mode === "stream") {
      es = new EventSource(logStreamPath(runId, source, trial));
      es.addEventListener("init", (ev) => {
        const d = JSON.parse((ev as MessageEvent).data) as { text: string; truncated: boolean; size: number };
        setText(d.text);
        setSize(d.size);
        setTruncated(d.truncated);
        setError(null);
      });
      es.addEventListener("append", (ev) => {
        const d = JSON.parse((ev as MessageEvent).data) as { text: string; size: number };
        setText((cur) => {
          const next = cur + d.text;
          return next.length > MAX_BUFFER ? next.slice(next.length - MAX_BUFFER) : next;
        });
        setSize(d.size);
      });
      es.addEventListener("end", () => {
        es?.close();
        // The server closes after its time limit; reopen while the run is live.
        timer = setTimeout(() => { if (!cancelled) setGeneration((g) => g + 1); }, 1000);
      });
      es.onerror = () => {
        es?.close();
        if (!cancelled) setFallback(true);
      };
    } else {
      void fetchOnce();
    }
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      es?.close();
    };
  }, [runId, source, trial, mode, generation]);

  useEffect(() => {
    if (follow && preRef.current) preRef.current.scrollTop = preRef.current.scrollHeight;
  }, [text, follow]);

  return (
    <div>
      <div className="text-xs">
        {size !== null && (
          <span className="text-zinc-500">
            {fmtBytes(size)}{truncated ? ", showing the tail" : ""}, redacted
          </span>
        )}
        {live && <span className="ml-3 text-zinc-600">{mode === "stream" ? "streaming" : "refreshes every 3 s"}</span>}
        {error && <span className="ml-3 text-amber-300">{error}</span>}
      </div>
      <pre
        ref={preRef}
        className="mt-2 h-[28rem] overflow-auto rounded-xl border border-white/10 bg-black p-4 text-xs leading-5 text-zinc-300 font-mono whitespace-pre-wrap break-all"
      >
        {text || (error ? "" : "loading")}
      </pre>
    </div>
  );
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
  // A choice the user picked wins while it still exists; otherwise the first one.
  const choice = choices.find((c) => c.key === key) ?? choices[0];

  if (choices.length === 0 || !choice) {
    return <div className="text-sm text-zinc-500">No log files on disk for this run yet.</div>;
  }

  return (
    <div>
      <div className="flex flex-wrap items-center gap-3 text-xs">
        <select
          value={choice.key}
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
      </div>
      <div className="mt-2">
        {/* The key remounts the viewer (fresh buffer and stream) when the chosen log changes. */}
        <Viewer key={`${runId}|${choice.key}`} runId={runId} source={choice.source} trial={choice.trial} live={live} follow={follow} />
      </div>
    </div>
  );
}
