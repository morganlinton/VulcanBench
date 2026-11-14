"use client";

import { useState } from "react";

// An in-page yes or no for the builder's irreversible steps (accept, drop,
// freeze, lock). It replaces window.confirm and window.prompt: the desktop
// app is Electron, which has no window.prompt, and an in-page question can
// carry a reason field and say what the step leaves out.
export function ConfirmBar({
  message,
  confirmLabel,
  withReason = false,
  busy = false,
  onConfirm,
  onCancel,
}: {
  message: React.ReactNode;
  confirmLabel: string;
  withReason?: boolean;
  busy?: boolean;
  onConfirm: (reason: string) => void;
  onCancel: () => void;
}) {
  const [reason, setReason] = useState("");
  return (
    <div
      role="alertdialog"
      aria-label={confirmLabel}
      className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-amber-100"
    >
      <span>{message}</span>
      {withReason && (
        <input
          aria-label="Reason (optional)"
          placeholder="reason (optional)"
          value={reason}
          maxLength={300}
          autoFocus
          onChange={(e) => setReason(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onConfirm(reason.trim());
            if (e.key === "Escape") onCancel();
          }}
          className="min-w-48 flex-1 rounded-md border border-white/15 bg-zinc-900 px-2 py-1 text-zinc-100"
        />
      )}
      <button
        type="button"
        onClick={() => onConfirm(reason.trim())}
        disabled={busy}
        autoFocus={!withReason}
        className="rounded-full border border-amber-400/50 px-3 py-0.5 text-amber-200 hover:bg-amber-500/10 disabled:opacity-40"
      >
        {busy ? "Working" : confirmLabel}
      </button>
      <button type="button" onClick={onCancel} disabled={busy} className="rounded-full border border-white/15 px-3 py-0.5 text-zinc-300 hover:bg-white/5 disabled:opacity-40">
        Cancel
      </button>
    </div>
  );
}
