"""Pilot: Muse Wonder 1.0 at Low on VulcanBench Frontier v4, Pi vs. Muse Code, both API-billed.

Same model, same tasks, same effort; only the harness differs. Each task runs
in Pi first, then in Muse Code, one run at a time. The pilot stops before the
next run once the summed API cost passes --budget. Pi also gets a per-run
kill cap (--pi-run-cap); Muse Code cannot cap mid-run, so its spend is bounded
by the budget check between runs and the 3-hour task timeout.

Not board data: no judging, one run per cell, run from a committed branch.

    python scripts/cii-v4-board/run_wonder_harness_pilot.py --budget 40
"""

import argparse
import json
import os
import sys
from datetime import UTC, datetime
from pathlib import Path

from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.agent.loop import run_agent  # noqa: E402

SUITE = ROOT / "tasks/coding-intelligence-index-v4"
OUT = ROOT / "runs-pilot-wonder-harness"
TASKS = [
    "legacy-qlite-store-parity",
    "legacy-matchcore-order-book-parity",
    "legacy-settlecore-binary-parity",
]
ARMS = {"pi": "pi:meta/muse-wonder-1.0", "muse-code": "muse-code:muse-wonder-1.0-api"}
PINS = {
    "VULCANBENCH_PI_NODE": str(Path.home() / ".local/node/bin/node"),
    "VULCANBENCH_PI_CLI": str(
        Path.home()
        / ".local/share/vulcanbench/pi-1.2.0/node_modules/@earendil-works/pi-coding-agent"
        / "dist/bundle/cli.js"
    ),
    "VULCANBENCH_PI_SHA256": "e79626f2dd6f94aa45d30f3fa63cd84319a6eefcd150b353cfaf274366926774",
    "VULCANBENCH_PI_CONFIG": str(Path.home() / ".local/share/vulcanbench/pi-config"),
    "VULCANBENCH_MUSE_BINARY": str(Path.home() / ".local/bin/muse-bin-1.4.6-R6000.1"),
    "VULCANBENCH_MUSE_SHA256": "c713165a287d64afb8816f8351a240b457a23524f4ade06bca0e41dfde410460",
}


def spent():
    total = 0.0
    for path in OUT.glob("*/low/*/summary.json"):
        cost = json.loads(path.read_text())["economics"]["marginal_cash_usd"]
        total += cost or 0.0
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--budget", type=float, required=True, help="Stop once API spend passes this"
    )
    parser.add_argument("--pi-run-cap", type=float, default=15.0)
    args = parser.parse_args()
    load_dotenv(ROOT / ".env")
    os.environ.update(PINS)
    os.environ["PATH"] = str(ROOT / ".venv/bin") + os.pathsep + os.environ.get("PATH", "")
    for task in TASKS:
        for arm, spec in ARMS.items():
            output = OUT / arm / "low"
            output.mkdir(parents=True, exist_ok=True)
            if any(
                json.loads(p.read_text())["task_id"] == task for p in output.glob("*/summary.json")
            ):
                continue
            before = spent()
            if before >= args.budget:
                print(f"STOP budget reached: ${before:.2f} >= ${args.budget:.2f}", flush=True)
                return
            print(
                f"START {datetime.now(UTC).isoformat()} {arm} {task} spent=${before:.2f}",
                flush=True,
            )
            result = run_agent(
                task,
                spec,
                output_dir=output,
                tasks_root=SUITE,
                judges=False,
                sandbox="local",
                network=False,
                suite="coding-intelligence-index-v4",
                effort="low",
                max_run_cost=args.pi_run_cap if arm == "pi" else None,
            )
            s = result["summary"]
            print(
                "RESULT "
                + json.dumps(
                    {
                        "arm": arm,
                        "task": task,
                        "functional": s["scores"]["functional"],
                        "usd": s["economics"]["marginal_cash_usd"],
                        "tokens": s["total_tokens"],
                        "minutes": round(s["duration_s"] / 60, 1),
                    }
                ),
                flush=True,
            )
    print(f"DONE spent=${spent():.2f}", flush=True)


if __name__ == "__main__":
    main()
