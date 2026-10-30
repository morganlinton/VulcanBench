"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { RunnerError } from "@/lib/runner";

export interface Polled<T> {
  data: T | null;
  error: RunnerError | null;
  loading: boolean;
  updatedAt: Date | null;
  refresh: () => void;
}

// Poll a fetcher on an interval. The interval can change between renders
// (faster while a run is active); 0 disables polling after the first load.
export function usePolling<T>(
  fetcher: () => Promise<T>,
  intervalMs: number,
  key: string,
): Polled<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<RunnerError | null>(null);
  const [loading, setLoading] = useState(true);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);
  const [tick, setTick] = useState(0);
  const fetcherRef = useRef(fetcher);

  // Keep the latest fetcher without restarting the poll loop on every render.
  useEffect(() => {
    fetcherRef.current = fetcher;
  });

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const run = async () => {
      try {
        const next = await fetcherRef.current();
        if (cancelled) return;
        setData(next);
        setError(null);
        setUpdatedAt(new Date());
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof RunnerError ? e : new RunnerError(0, String(e)));
      } finally {
        if (!cancelled) setLoading(false);
        if (!cancelled && intervalMs > 0) timer = setTimeout(run, intervalMs);
      }
    };

    void run();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [intervalMs, key, tick]);

  return { data, error, loading, updatedAt, refresh };
}
