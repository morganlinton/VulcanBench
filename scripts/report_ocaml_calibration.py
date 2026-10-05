#!/usr/bin/env python3
"""Report OCaml pilot calibration without treating partial coverage as a suite score."""

from __future__ import annotations

import argparse
import base64
import html
import json
from pathlib import Path
from typing import Any

from harness.compare import suite_version
from harness.tasks import load_task


def trace_only_invocations(
    runs_root: Path, task_ids: list[str], model: str
) -> list[dict[str, Any]]:
    """Expose aborted invocations without admitting unverifiable scoring identities."""
    receipts = []
    for trace in sorted(runs_root.glob("*/trace.jsonl")):
        if (trace.parent / "summary.json").is_file():
            continue
        try:
            with trace.open() as stream:
                start = json.loads(stream.readline())
        except (OSError, json.JSONDecodeError):
            continue
        if start.get("model") != model or start.get("task_id") not in task_ids:
            continue
        receipts.append(
            {
                "run_id": trace.parent.name,
                "task_id": start["task_id"],
                "model": model,
                "started_at": start.get("ts"),
                "reason": "No summary or functional verdict; effort, revision, and environment require separate audit.",
                "counts_toward_coverage": False,
            }
        )
    return receipts


def budget_exclusion(summary: dict[str, Any], metadata: dict[str, Any]) -> str | None:
    """Keep ablations and changed solve limits out of the standard OCaml column."""
    if summary.get("budget_override"):
        return "budget override ablation; not standard run conditions"
    limits = summary.get("run_limits") or {}
    hints = metadata.get("agent_hints") or {}
    for field, hint in (
        ("agent_timeout_s", "suggested_timeout_s"),
        ("configured_max_steps", "suggested_max_steps"),
    ):
        if (
            limits.get(field) is not None
            and hints.get(hint) is not None
            and limits[field] != hints[hint]
        ):
            return f"different solve limits: {field}={limits[field]}; standard {hints[hint]}"
    return None


def eligibility_exclusion(run: Path) -> str | None:
    """Honor a saved review decision without changing the raw solver receipt."""
    path = run / "CALIBRATION_ELIGIBILITY.json"
    if not path.is_file():
        return None
    review = json.loads(path.read_text())
    if review.get("eligible") is True:
        return None
    return str(review.get("reason", "calibration eligibility review required"))


def build_report(
    runs_root: Path,
    *,
    tasks_base: Path = Path("tasks"),
    suite: str = "ocaml-v1",
    model: str = "codex:gpt-6.1-sol",
    effort: str = "medium",
    repetitions: int = 3,
    sandbox: str | None = None,
) -> dict[str, Any]:
    """Include every matching fresh attempt; exclude stale definitions explicitly."""
    if repetitions < 1:
        raise ValueError("repetitions must be positive")
    if sandbox not in {None, "local", "docker"}:
        raise ValueError("sandbox must be local or docker")
    identity = suite_version(suite, tasks_base)
    metadata_by_task = {
        name: load_task(name, tasks_base / suite).metadata for name in identity["task_ids"]
    }
    by_task: dict[str, list[dict[str, Any]]] = {name: [] for name in identity["task_ids"]}
    excluded: list[dict[str, str]] = []
    unscored: list[dict[str, Any]] = []
    for path in sorted(runs_root.glob("*/summary.json")):
        summary = json.loads(path.read_text())
        name = summary.get("task_id")
        if name not in by_task or summary.get("model") != model:
            continue
        if (summary.get("effort") or {}).get("requested") != effort:
            continue
        reason = eligibility_exclusion(path.parent) or budget_exclusion(
            summary, metadata_by_task[name]
        )
        runtime = ((summary.get("manifest") or {}).get("sandbox") or {}).get("mode", "unknown")
        if summary.get("task_hash") != identity["task_hashes"][name]:
            reason = "old task revision"
        elif (summary.get("integrity_audit") or {}).get("contaminated"):
            reason = "integrity contamination; requires review"
        elif sandbox is not None and runtime != sandbox:
            reason = f"different execution environment: {runtime}; selected {sandbox}"
        if reason:
            excluded.append({"run_id": path.parent.name, "reason": reason})
            continue
        if (summary.get("scores") or {}).get("functional") is None:
            unscored.append(
                {
                    "run_id": path.parent.name,
                    "task_id": name,
                    "task_hash": summary["task_hash"],
                    "reason": "no scored functional receipt; does not satisfy coverage",
                    "agent_error": summary.get("agent_error"),
                    "cli_agent": summary.get("cli_agent"),
                }
            )
            continue
        by_task[name].append(
            {
                "run_id": path.parent.name,
                "task_hash": summary["task_hash"],
                "passed": (summary.get("scores") or {}).get("functional") == 1.0,
                "functional": (summary.get("scores") or {}).get("functional"),
                "verifier": summary.get("verifier"),
                "failed_checks": {
                    group: [
                        check
                        for check, passed in (
                            (summary.get("verifier") or {}).get(group) or {}
                        ).items()
                        if not passed
                    ]
                    for group in ("fail_to_pass", "pass_to_pass")
                },
                "agent_error": summary.get("agent_error"),
                "duration_s": summary.get("duration_s"),
                "tokens": summary.get("tokens"),
                "cli_agent": summary.get("cli_agent"),
                "manifest": summary.get("manifest"),
                "sandbox": runtime,
                "run_limits": summary.get("run_limits"),
            }
        )
    tasks = []
    for name, attempts in by_task.items():
        metadata = metadata_by_task[name]
        passed = sum(attempt["passed"] for attempt in attempts)
        tasks.append(
            {
                "task_id": name,
                "primary_view": metadata.get("primary_view"),
                "source": metadata.get("source"),
                "decontaminated": metadata.get("decontaminated"),
                "decontamination_notes": metadata.get("decontamination_notes"),
                "upstream": metadata.get("upstream"),
                "passes": passed,
                "attempts": len(attempts),
                "pass_at_1": passed / len(attempts) if attempts else None,
                "runs": attempts,
            }
        )
    complete = all(row["attempts"] >= repetitions for row in tasks)
    runtimes = sorted({attempt["sandbox"] for row in tasks for attempt in row["runs"]})
    homogeneous = len(runtimes) <= 1
    score = (
        sum(row["pass_at_1"] for row in tasks) / len(tasks) if complete and homogeneous else None
    )
    views = {}
    for view in ("engineering", "language-mastery"):
        members = [row for row in tasks if row["primary_view"] in (view, "both")]
        view_complete = (
            bool(members) and homogeneous and all(row["attempts"] >= repetitions for row in members)
        )
        views[view] = {
            "task_ids": [row["task_id"] for row in members],
            "distinct_tasks": len(members),
            "passes": sum(row["passes"] for row in members),
            "attempts": sum(row["attempts"] for row in members),
            "coverage_complete": view_complete,
            "pass_at_1": (
                sum(row["pass_at_1"] for row in members) / len(members) if view_complete else None
            ),
        }
    return {
        **identity,
        "status": (
            "mixed execution environments; select one"
            if not homogeneous
            else "development calibration complete"
            if complete
            else "incomplete calibration"
        ),
        "model": model,
        "effort": effort,
        "minimum_attempts_per_task": repetitions,
        "sandbox_selection": sandbox,
        "execution_environments": runtimes,
        "metric": "mean per-task pass@1; all required checks and regression guards must pass",
        "pass_at_1": score,
        "observed_below_80_percent": score < 0.8 if score is not None else None,
        "independent_confirmation": False,
        "views": views,
        "view_overlap": "Tasks tagged both enter each view but are counted once in the suite.",
        "limitations": [
            f"{len(tasks)} tasks are a development pool, not a stable leaderboard or a confidence claim.",
            "Configured model identity may be requested-only; inspect each CLI receipt.",
            "Development selection and retrospective audits are not independent confirmation.",
            "Public upstream patches may be retrievable; task provenance is not a training-cutoff claim.",
            (
                "Docker calibration on this Mac uses Linux amd64 emulation; timing is diagnostic."
                if runtimes == ["docker"]
                else "Native runs execute on the host without Docker resource caps or network isolation."
                if runtimes == ["local"]
                else "Execution environment is missing or mixed; inspect individual receipts."
            ),
            "Quality and security analyzers do not currently score OCaml.",
            "Historical summaries may lack effective run limits; audit their launch receipts before publishing.",
        ],
        "excluded_runs": excluded,
        "unscored_runs": unscored,
        "trace_only_invocations": trace_only_invocations(runs_root, identity["task_ids"], model),
        "tasks": tasks,
    }


def render_model_card(report: dict[str, Any]) -> str:
    """Render a self-contained functional-results card with visible coverage limits."""
    root = Path(__file__).resolve().parents[1]

    def asset(path: str) -> str:
        return base64.b64encode((root / path).read_bytes()).decode()

    def escape(value: Any) -> str:
        return html.escape(str(value))

    def percent(value: float | None) -> str:
        return "Pending coverage" if value is None else f"{value:.1%}"

    attempts = sum(row["attempts"] for row in report["tasks"])
    passes = sum(row["passes"] for row in report["tasks"])
    covered = sum(row["attempts"] >= report["minimum_attempts_per_task"] for row in report["tasks"])
    rows = "".join(
        f"<tr><td>{escape(row['task_id'])}</td><td>{escape(row['primary_view'])}</td>"
        f"<td>{row['passes']}/{row['attempts']}</td><td>{percent(row['pass_at_1'])}</td></tr>"
        for row in report["tasks"]
    )
    views = "".join(
        f"<li>{escape(name)}: {percent(view['pass_at_1'])} "
        f"({view['distinct_tasks']} distinct tasks, {view['attempts']} attempts)</li>"
        for name, view in report["views"].items()
    )
    limits = "".join(f"<li>{escape(note)}</li>" for note in report["limitations"])
    hashes = "\n".join(f"{name}: {value}" for name, value in report["task_hashes"].items())
    provenance = "".join(
        f"<li>{escape(row['task_id'])}: {escape(row['source'])}; "
        f"decontaminated={escape(row['decontaminated'])}. "
        f"{escape(row['decontamination_notes'] or 'No provenance note recorded.')}</li>"
        for row in report["tasks"]
    )
    environments = ", ".join(report["execution_environments"]) or "No matching scored receipts"
    mono = asset("scripts/rankings-chart/ibm-plex-mono-400.ttf")
    chakra = asset("scripts/rankings-chart/chakra-600.ttf")
    logo = asset("docs/assets/vulcanbench-logo.png")
    licenses = "\n\n".join(
        (root / f"scripts/rankings-chart/{name}").read_text()
        for name in ("OFL-Chakra-Petch.txt", "OFL-IBM-Plex-Mono.txt")
    )
    suite_name = {
        "ocaml-v1": "OCaml v1",
        "ocaml-candidates": "OCaml candidates",
        "ocaml-incremental-candidate": "OCaml Incremental candidate",
        "ocaml-atomic-candidate": "OCaml atomic rewiring candidate",
        "ocaml-atomic-candidate-v2": "OCaml atomic rewiring candidate v2",
        "ocaml-serialization-candidate": "OCaml typed serialization candidate",
        "ocaml-serialization-candidate-v2": "OCaml typed serialization candidate v2",
        "ocaml-serialization-candidate-v3": "OCaml typed serialization candidate v3",
        "ocaml-union-heldout": "OCaml open union held-out workload",
        "ocaml-registry-candidate": "OCaml generative module registry candidate",
    }.get(report["suite"], report["suite"])
    return f"""<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>VulcanBench {escape(suite_name)}: {escape(report["model"])}</title>
<style>
@font-face{{font-family:Chakra;src:url(data:font/ttf;base64,{chakra})}}
@font-face{{font-family:Plex;src:url(data:font/ttf;base64,{mono})}}
*{{box-sizing:border-box}}body{{margin:0;background:#eee;color:#0b0b0b;font:14px Plex,monospace}}
main{{max-width:1100px;margin:32px auto;background:white;padding:44px}}
header{{display:flex;align-items:center;gap:14px;border-bottom:2px solid;padding-bottom:20px}}
header img{{width:52px;border-radius:22%}}header b{{font:28px Chakra,sans-serif}}
h1,h2{{font-family:Chakra,sans-serif}}h1{{font-size:32px;margin-bottom:8px}}
.score{{font:42px Chakra,sans-serif;margin:20px 0}}.meta{{line-height:1.8}}
table{{width:100%;border-collapse:collapse;margin:24px 0}}th,td{{text-align:left;padding:12px 8px;border-bottom:1px solid #bbb}}
li,p{{line-height:1.6}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;font:11px Plex,monospace}}
details{{margin-top:24px}}@media(max-width:700px){{main{{margin:0;padding:20px}}table{{font-size:11px}}}}
@media print{{body{{background:white}}main{{margin:0}}}}
</style><main>
<header><img alt="VulcanBench logo" src="data:image/png;base64,{logo}"><b>VulcanBench</b></header>
<h1>{escape(suite_name)} model card</h1><p>{escape(report["model"])}, effort {escape(report["effort"])}</p>
<div class="score">Task pass@1: {percent(report["pass_at_1"])}</div>
<div class="meta">Development suite {escape(report["version"])}<br>
Status: {escape(report["status"])}<br>
Coverage: {covered}/{report["n_tasks"]} tasks with at least {report["minimum_attempts_per_task"]} attempts<br>
Observed complete passes: {passes}/{attempts} scored attempts<br>
Execution: {escape(environments)}<br>
Unscored summary receipts: {len(report["unscored_runs"])}; excluded receipts: {len(report["excluded_runs"])}<br>
Trace-only invocations needing identity audit: {len(report["trace_only_invocations"])}</div>
<table><thead><tr><th>Task</th><th>View</th><th>Passes/attempts</th><th>Task pass@1</th></tr></thead><tbody>{rows}</tbody></table>
<h2>Coverage views</h2><ul>{views}</ul><p>{escape(report["view_overlap"])}</p>
<h2>Method and limitations</h2>
<p>{escape(report["metric"])}. Independent confirmation: {escape(report["independent_confirmation"])}.
This card reports functional task completion. It does not report quality, security, a composite index,
or statistical confidence. The complete JSON report retains individual run identities, effective solve limits,
and receipts. Configured step allowances may not be enforced as CLI turn limits.</p>
<ul>{limits}</ul>
<details><summary>Task provenance</summary><ul>{provenance}</ul></details>
<details><summary>Frozen task hashes</summary><pre>{escape(hashes)}</pre></details>
<details><summary>Embedded font credits and licenses</summary><pre>{escape(licenses)}</pre></details>
</main></html>"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs-root", type=Path, default=Path("runs-ocaml-pilot"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--model", default="codex:gpt-6.1-sol", help="Full recorded model identity")
    parser.add_argument("--effort", default="medium")
    parser.add_argument(
        "--suite",
        choices=(
            "ocaml-v1",
            "ocaml-candidates",
            "ocaml-incremental-candidate",
            "ocaml-atomic-candidate",
            "ocaml-atomic-candidate-v2",
            "ocaml-serialization-candidate",
            "ocaml-serialization-candidate-v2",
            "ocaml-serialization-candidate-v3",
            "ocaml-union-heldout",
            "ocaml-registry-candidate",
        ),
        default="ocaml-v1",
    )
    parser.add_argument("--sandbox", choices=("local", "docker"))
    parser.add_argument("--card", type=Path, help="Also write a self-contained HTML model card")
    args = parser.parse_args()
    report = build_report(
        args.runs_root, suite=args.suite, model=args.model, effort=args.effort, sandbox=args.sandbox
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    if args.card:
        args.card.parent.mkdir(parents=True, exist_ok=True)
        args.card.write_text(render_model_card(report))
    print(report["status"])
    for task in report["tasks"]:
        print(f"{task['task_id']}: {task['passes']}/{task['attempts']} complete passes")
    if report["pass_at_1"] is not None:
        print(f"Task pass@1: {report['pass_at_1']:.1%}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
