"use client";

import Link from "next/link";
import { createContext, useContext, useRef, useState } from "react";
import { RunnerUnavailable } from "@/components/runner/RunnerUnavailable";
import { button, field, label } from "@/components/runner/launchShared";
import { usePolling } from "@/components/runner/usePolling";
import { RunnerError } from "@/lib/runner";
import { getAccess, isLocked, postLock, postUnlock, type AccessResponse } from "@/lib/suites";

interface Gate {
  /** Call after any request that came back 403 builder_locked: re-reads access. */
  recheck: () => void;
  /** Report an error; a lock error re-reads access, anything else is returned as text. */
  handle: (e: unknown) => string;
  access: AccessResponse;
}

const GateContext = createContext<Gate | null>(null);

export function useGate(): Gate {
  const g = useContext(GateContext);
  if (!g) throw new Error("useGate outside BuilderGate");
  return g;
}

export function errorText(e: unknown): string {
  if (e instanceof RunnerError) {
    const hint = e.detail && typeof e.detail.hint === "string" ? ` (${e.detail.hint})` : "";
    return `${e.message}${hint}`;
  }
  return e instanceof Error ? e.message : String(e);
}

// The Suites pages render only when the builder is unlocked on this Mac. The
// check is `vulcanbench suite access --json` in the tooling worktree; the
// engine enforces the same gate on every command, so this screen is a
// convenience, not the protection.
export function BuilderGate({ children }: { children: React.ReactNode }) {
  const { data, error, refresh } = usePolling<AccessResponse>(getAccess, 0, "access");
  if (error && !data) return <RunnerUnavailable error={error} />;
  if (!data) return <div className="mt-6 text-sm text-zinc-500">checking the suite builder</div>;
  if (!data.tooling.ready || !data.access) return <NoTooling data={data} />;
  if (!data.access.configured) return <NotConfigured tag={data.tooling.tag} />;
  if (!data.access.unlocked) return <UnlockForm onUnlocked={refresh} />;
  const gate: Gate = {
    recheck: refresh,
    handle: (e) => {
      if (isLocked(e)) refresh();
      return errorText(e);
    },
    access: data,
  };
  return (
    <GateContext.Provider value={gate}>
      <LockBar data={data} onLocked={refresh} />
      {children}
    </GateContext.Provider>
  );
}

/** An error to show while rendering; a lock error offers the unlock screen (no state change in render). */
export function ErrorLine({ error }: { error: unknown }) {
  const gate = useContext(GateContext);
  return (
    <div className="mt-4 text-sm text-rose-300">
      {errorText(error)}
      {isLocked(error) && gate && (
        <button type="button" onClick={gate.recheck} className="ml-3 rounded-full border border-white/15 px-3 py-0.5 text-xs text-zinc-300 hover:bg-white/5">
          Unlock again
        </button>
      )}
    </div>
  );
}

function NoTooling({ data }: { data: AccessResponse }) {
  return (
    <div className="mt-6 rounded-xl border border-amber-500/40 bg-amber-500/5 p-4 text-sm text-amber-200 space-y-2">
      <div className="font-medium">The suite builder needs a prepared bench tag</div>
      <p className="text-amber-200/80">
        Its commands run from a run worktree pinned to a bench tag, like every benchmark run. Cut a tag from
        main that contains the suite builder, then prepare its worktree on the Launch page (no model calls).
      </p>
      <ul className="list-disc pl-5 text-xs text-amber-200/80">
        {data.tooling.problems.map((p) => <li key={p}>{p}</li>)}
      </ul>
      <Link href="/launch" className="inline-block rounded-full border border-emerald-500/40 px-4 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10">
        Go to Launch
      </Link>
    </div>
  );
}

function NotConfigured({ tag }: { tag: string | null }) {
  return (
    <div className="mt-6 rounded-xl border border-white/10 p-5 text-sm text-zinc-300 space-y-3 max-w-2xl">
      <div className="text-lg font-medium text-white">The suite builder is for subscribers</div>
      <p>
        No suite builder password is set at {tag ?? "this bench tag"}, so the builder stays locked. The owner
        sets one in a terminal on this Mac:
      </p>
      <pre className="rounded-md bg-zinc-900 p-3 font-mono text-xs text-zinc-200">.venv/bin/vulcanbench suite password-hash</pre>
      <p className="text-zinc-400">
        It asks for the password twice and prints only its hash. Put the hash in{" "}
        <code className="font-mono">[builder].password_hash</code> in <code className="font-mono">vulcanbench.toml</code>,
        commit it, cut a new bench tag and prepare it. The password itself never goes into a file.
      </p>
    </div>
  );
}

function UnlockForm({ onUnlocked }: { onUnlocked: () => void }) {
  // Uncontrolled on purpose: the value is read once in the submit handler and
  // the field is cleared at once, so the password never sits in React state.
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    const el = input.current;
    if (!el || !el.value) return;
    const password = el.value;
    el.value = "";
    setBusy(true);
    setError(null);
    try {
      await postUnlock(password);
      onUnlocked();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
      el.focus();
    }
  };
  return (
    <form onSubmit={submit} className="mt-6 max-w-md rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.03] p-5 space-y-4">
      <div>
        <div className="text-lg font-medium">Unlock the suite builder</div>
        <p className="mt-1 text-sm text-zinc-400">
          Building your own eval suite is for subscribers. Enter the subscriber password once; this Mac stays
          unlocked until you lock it again or the password changes.
        </p>
      </div>
      <div>
        <label htmlFor="builder-password" className={label}>Password</label>
        <input
          id="builder-password"
          ref={input}
          type="password"
          name="builder-password"
          autoComplete="current-password"
          autoFocus
          spellCheck={false}
          className={field}
        />
      </div>
      {error && <div className="text-xs text-rose-300">{error}</div>}
      <button type="submit" disabled={busy} className={`${button} bg-emerald-500 text-black hover:bg-emerald-400`}>
        {busy ? "Checking" : "Unlock"}
      </button>
      <p className="text-xs text-zinc-500">
        The password goes from this window to the VulcanBench backend on this Mac (127.0.0.1), and from there
        to <code className="font-mono">vulcanbench suite unlock</code> on standard input. It is not saved,
        logged or sent anywhere else; this Mac keeps only a fingerprint of the configured hash.
      </p>
    </form>
  );
}

function LockBar({ data, onLocked }: { data: AccessResponse; onLocked: () => void }) {
  const [busy, setBusy] = useState(false);
  const lock = async () => {
    if (!window.confirm("Lock the suite builder on this Mac? Unlocking again needs the password.")) return;
    setBusy(true);
    try {
      await postLock();
    } finally {
      setBusy(false);
      onLocked();
    }
  };
  return (
    <div className="mt-4 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/10 px-3 py-2 text-xs text-zinc-400">
      <span>
        Suite builder unlocked{data.access?.unlocked_at ? ` since ${data.access.unlocked_at}` : ""}. Commands run from{" "}
        <span className="font-mono text-zinc-300">{data.tooling.tag}</span>.
      </span>
      <button type="button" onClick={lock} disabled={busy} className="rounded-full border border-white/15 px-3 py-0.5 hover:bg-white/5">
        Lock
      </button>
    </div>
  );
}
