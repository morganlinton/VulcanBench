import { API_BASE } from "@/lib/api";
import type { RunnerError } from "@/lib/runner";

export function RunnerUnavailable({ error }: { error: RunnerError }) {
  let hint: string;
  if (error.status === 0) {
    hint = `The backend at ${API_BASE} did not answer. Start it with "make gui-backend".`;
  } else if (error.status === 404) {
    hint = "The runner is disabled on this backend. Start it with VULCANBENCH_RUNNER=1 (make gui-backend does).";
  } else if (error.status === 403) {
    hint = "The runner answers loopback clients only. Open the dashboard on this machine at 127.0.0.1.";
  } else {
    hint = error.message;
  }
  return (
    <div className="mt-6 border border-amber-500/40 bg-amber-500/5 rounded-xl p-4 text-sm text-amber-200">
      <div className="font-medium">Runner unavailable</div>
      <div className="mt-1 text-amber-200/80">{hint}</div>
    </div>
  );
}
