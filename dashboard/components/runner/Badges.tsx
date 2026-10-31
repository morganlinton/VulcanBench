import type { FirstCheck } from "@/lib/runner";
import { isActive, statusLabel } from "@/lib/runner";

const STATUS_CLASS: Record<string, string> = {
  external_running: "border-emerald-500/40 text-emerald-400",
  running: "border-emerald-500/40 text-emerald-400",
  preparing: "border-sky-500/40 text-sky-300",
  queued: "border-sky-500/40 text-sky-300",
  waiting: "border-violet-500/40 text-violet-300",
  stopping: "border-amber-500/40 text-amber-300",
  finished: "border-white/15 text-zinc-200",
  incomplete: "border-amber-500/40 text-amber-300",
  stopped: "border-amber-500/40 text-amber-300",
  infra_failed: "border-rose-500/40 text-rose-300",
  failed: "border-rose-500/40 text-rose-300",
  lost: "border-rose-500/40 text-rose-300",
  unknown: "border-white/10 text-zinc-500",
};

export function StatusBadge({ status }: { status: string }) {
  const cls = STATUS_CLASS[status] ?? STATUS_CLASS.unknown;
  return (
    <span className={`inline-flex items-center gap-2 rounded-full border px-2.5 py-0.5 text-xs ${cls}`}>
      {isActive(status) && <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />}
      {statusLabel(status)}
    </span>
  );
}

const CHECK_CLASS: Record<FirstCheck["status"], string> = {
  ok: "bg-emerald-500/15 text-emerald-300 border-emerald-500/30",
  pending: "bg-sky-500/15 text-sky-300 border-sky-500/30",
  warning: "bg-amber-500/15 text-amber-300 border-amber-500/30",
  fail: "bg-rose-500/15 text-rose-300 border-rose-500/30",
  unknown: "bg-white/5 text-zinc-500 border-white/10",
};

const CHECK_LABEL: Record<FirstCheck["status"], string> = {
  ok: "session ok",
  pending: "checking",
  warning: "check warning",
  fail: "check failed",
  unknown: "no evidence",
};

export function FirstCheckBadge({ check, compact = false }: { check: FirstCheck | null; compact?: boolean }) {
  if (!check) return <span className="text-xs text-zinc-600">n/a</span>;
  const cls = CHECK_CLASS[check.status] ?? CHECK_CLASS.unknown;
  const title = check.reasons.join("; ");
  return (
    <span className={`inline-flex items-center rounded-md border px-2 py-0.5 text-xs ${cls}`} title={title}>
      {CHECK_LABEL[check.status] ?? check.status}
      {!compact && check.tool_calls > 0 && (
        <span className="ml-1.5 text-zinc-400">{check.tool_calls} tools</span>
      )}
    </span>
  );
}

// One dot per trial: solved, not solved, running, or something else.
export function RewardDots({
  trials,
}: {
  trials: { id: string; status: string; reward: Record<string, number | string | null> | null }[];
}) {
  if (trials.length === 0) return <span className="text-zinc-600">none yet</span>;
  return (
    <span className="inline-flex items-center gap-1.5">
      {trials.map((t) => {
        const r = t.reward?.reward;
        let cls = "bg-zinc-700";
        let label = `${t.id}: ${t.status}`;
        if (t.status === "running") {
          cls = "bg-emerald-400 animate-pulse";
        } else if (typeof r === "number") {
          cls = r >= 1 ? "bg-emerald-400" : "bg-zinc-500";
          label = `${t.id}: reward ${r}`;
        } else if (t.status === "cancelled") {
          cls = "bg-amber-400/70";
        } else if (t.status === "error" || t.status === "interrupted") {
          cls = "bg-rose-400";
        }
        return <span key={t.id} title={label} className={`inline-block h-2.5 w-2.5 rounded-full ${cls}`} />;
      })}
    </span>
  );
}
