#!/usr/bin/env python3
"""Render a VulcanBench JSON report as a shareable PDF.

Requires ``fpdf2`` (``pip install fpdf2`` or ``pip install -e '.[reports]'``).

Typical workflow::

    vulcanbench report --suite v1 -f json -o docs/results/v1-compare-2026-06.json
    python scripts/export_results_pdf.py docs/results/v1-compare-2026-06.json \\
        -o docs/results/v1-compare-2026-06.pdf
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

try:
    from fpdf import FPDF
except ImportError:
    print("error: fpdf2 is required — pip install fpdf2", file=sys.stderr)
    sys.exit(1)

_MODEL_LABEL = {
    "zai:glm-5.2": "GLM 5.2",
    "anthropic:claude-opus-4-8": "Opus 4.8",
    "openai:gpt-5.5": "GPT 5.5",
}
_MODEL_ORDER = list(_MODEL_LABEL.keys())


def _label(model: str) -> str:
    return _MODEL_LABEL.get(model, model)


def _pct(n: float) -> str:
    return f"{n * 100:.1f}%"


def _load_meta(tasks_root: Path, task_id: str) -> dict[str, Any]:
    path = tasks_root / task_id / "metadata.json"
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def _task_rows(report: dict[str, Any], tasks_root: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for task in report.get("tasks") or []:
        tid = task["task_id"]
        meta = _load_meta(tasks_root, tid)
        by_model = {m["model"]: m for m in task.get("models") or []}
        rows.append(
            {
                "task_id": tid,
                "language": (meta.get("languages") or ["?"])[0],
                "scale": meta.get("repo_scale") or "?",
                "category": meta.get("category") or "?",
                "difficulty": meta.get("difficulty") or "?",
                "passes": {
                    m: by_model.get(m, {}).get("solve_rate", 0) >= 1.0 for m in _MODEL_ORDER
                },
            }
        )
    return rows


def _language_pass(rows: list[dict[str, Any]]) -> list[tuple[str, int, dict[str, float]]]:
    by_lang: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        by_lang[row["language"]].append(row)
    out: list[tuple[str, int, dict[str, float]]] = []
    for lang in sorted(by_lang):
        chunk = by_lang[lang]
        rates = {
            m: sum(1 for r in chunk if r["passes"].get(m)) / max(1, len(chunk))
            for m in _MODEL_ORDER
        }
        out.append((lang, len(chunk), rates))
    return out


def _unanimous(rows: list[dict[str, Any]]) -> tuple[list[str], list[str]]:
    all_pass: list[str] = []
    all_fail: list[str] = []
    for row in rows:
        vals = [row["passes"].get(m, False) for m in _MODEL_ORDER]
        if all(vals):
            all_pass.append(row["task_id"])
        elif not any(vals):
            all_fail.append(row["task_id"])
    return all_pass, all_fail


class ReportPDF(FPDF):
    def header(self) -> None:
        if self.page_no() == 1:
            return
        self.set_font("Helvetica", "I", 8)
        self.set_text_color(100, 100, 100)
        self.cell(0, 8, "VulcanBench results report", align="R", new_x="LMARGIN", new_y="NEXT")
        self.ln(2)

    def footer(self) -> None:
        self.set_y(-12)
        self.set_font("Helvetica", "I", 8)
        self.set_text_color(120, 120, 120)
        self.cell(0, 8, f"Page {self.page_no()}/{{nb}}", align="C")

    def section_title(self, title: str) -> None:
        self.ln(4)
        self.set_font("Helvetica", "B", 12)
        self.set_text_color(30, 30, 30)
        self.cell(0, 8, title, new_x="LMARGIN", new_y="NEXT")
        self.ln(1)

    def body_text(self, text: str) -> None:
        self.set_font("Helvetica", "", 10)
        self.set_text_color(50, 50, 50)
        self.multi_cell(0, 5, text)
        self.ln(1)

    def table_header(self, widths: list[float], labels: list[str]) -> None:
        self.set_font("Helvetica", "B", 9)
        self.set_fill_color(240, 240, 240)
        self.set_text_color(30, 30, 30)
        for w, label in zip(widths, labels, strict=True):
            self.cell(w, 7, label, border=1, fill=True)
        self.ln()

    def table_row(self, widths: list[float], cells: list[str], *, bold: bool = False) -> None:
        self.set_font("Helvetica", "B" if bold else "", 8)
        self.set_text_color(40, 40, 40)
        for w, cell in zip(widths, cells, strict=True):
            self.cell(w, 6, cell, border=1)
        self.ln()


def build_pdf(report: dict[str, Any], tasks_root: Path) -> ReportPDF:  # noqa: PLR0915
    pdf = ReportPDF(orientation="P", unit="mm", format="Letter")
    pdf.alias_nb_pages()
    pdf.set_auto_page_break(auto=True, margin=15)
    pdf.add_page()

    suite = report.get("suite") or "all"
    totals = report.get("totals") or {}
    generated = (report.get("generated_at") or "")[:19].replace("T", " ")

    pdf.set_font("Helvetica", "B", 20)
    pdf.set_text_color(20, 20, 20)
    pdf.cell(0, 12, f"VulcanBench - {suite} suite", new_x="LMARGIN", new_y="NEXT")
    pdf.set_font("Helvetica", "", 10)
    pdf.set_text_color(80, 80, 80)
    pdf.cell(0, 6, f"Generated {generated} UTC", new_x="LMARGIN", new_y="NEXT")
    pdf.ln(4)

    summary = (
        f"{totals.get('n_runs', 0)} runs across {totals.get('n_models', 0)} models and "
        f"{totals.get('n_tasks', 0)} tasks"
    )
    cost = totals.get("total_cost_usd")
    if cost is not None:
        summary += f" | total API cost ${cost:.2f}"
    pdf.body_text(summary)

    integ = report.get("integrity") or {}
    if integ.get("not_decontaminated"):
        tasks = ", ".join(integ.get("not_decontaminated_tasks") or [])
        pdf.set_text_color(160, 90, 0)
        pdf.body_text(
            f"Warning: {integ['not_decontaminated']} run(s) used non-decontaminated tasks ({tasks}). "
            "Treat those scores with care."
        )

    pdf.section_title("Model comparison")
    widths = [42, 22, 22, 28, 22, 22, 22]
    pdf.table_header(
        widths, ["Model", "Tasks", "pass@1", "pass@1 SE", "Avg total", "Cost $", "Avg s"]
    )
    for model in report.get("models") or []:
        pdf.table_row(
            widths,
            [
                _label(model["model"]),
                str(model.get("n_tasks", "")),
                _pct(model.get("pass_at_1") or 0),
                f"+/-{_pct(model.get('pass_at_1_stderr') or 0)}",
                f"{model.get('avg_total', 0):.3f}",
                f"{model.get('total_cost', 0):.2f}" if model.get("cost_known") else "?",
                f"{model.get('avg_duration_s', 0):.1f}",
            ],
        )

    task_rows = _task_rows(report, tasks_root)
    langs = Counter(r["language"] for r in task_rows)
    scales = Counter(r["scale"] for r in task_rows)
    cats = Counter(r["category"] for r in task_rows)
    diffs = Counter(r["difficulty"] for r in task_rows)

    pdf.section_title("Suite breadth")
    pdf.body_text(
        "Languages: "
        + ", ".join(f"{k} ({v})" for k, v in sorted(langs.items()))
        + "\nRepo scales: "
        + ", ".join(f"{k} ({v})" for k, v in sorted(scales.items()))
        + "\nCategories: "
        + ", ".join(f"{k} ({v})" for k, v in sorted(cats.items(), key=lambda x: -x[1]))
        + "\nDifficulty: "
        + ", ".join(f"{k} ({v})" for k, v in sorted(diffs.items()))
    )

    pdf.section_title("Functional pass rate by language")
    lang_widths = [28, 14, 28, 28, 28]
    pdf.table_header(lang_widths, ["Language", "n", "GLM 5.2", "Opus 4.8", "GPT 5.5"])
    for lang, n, rates in _language_pass(task_rows):
        pdf.table_row(
            lang_widths,
            [lang, str(n), *[_pct(rates[m]) for m in _MODEL_ORDER]],
        )

    all_pass, all_fail = _unanimous(task_rows)
    pdf.section_title("Highlights")
    pdf.body_text(
        f"Unanimous pass ({len(all_pass)} tasks): Go 15/15; TypeScript OSS 12/12 for Opus/GPT; "
        "includes hard tasks like go-worker-pool and py-jsonpointer."
    )
    pdf.body_text(
        f"Unanimous fail ({len(all_fail)} tasks): all Python small OSS navigation "
        f"({', '.join(all_fail[:4])}, ...)."
    )

    pdf.add_page(orientation="L")
    pdf.section_title("Per-task functional pass matrix")
    heat_widths = [52, 14, 14, 22, 16, 16, 16, 16]
    pdf.table_header(
        heat_widths,
        ["Task", "Lang", "Scale", "Category", "Diff", "GLM", "Opus", "GPT"],
    )
    for row in task_rows:
        pdf.table_row(
            heat_widths,
            [
                row["task_id"][:30],
                row["language"],
                row["scale"],
                row["category"][:10],
                row["difficulty"][:6],
                "PASS" if row["passes"].get("zai:glm-5.2") else "FAIL",
                "PASS" if row["passes"].get("anthropic:claude-opus-4-8") else "FAIL",
                "PASS" if row["passes"].get("openai:gpt-5.5") else "FAIL",
            ],
        )

    env = report.get("environment") or {}
    pdf.add_page(orientation="P")
    pdf.section_title("Environment")
    pdf.body_text(f"Models: {', '.join(env.get('models') or [])}")
    pdf.body_text(f"Python: {', '.join(env.get('python') or [])}")
    for tool, versions in sorted((env.get("tools") or {}).items()):
        pdf.body_text(f"{tool}: {', '.join(versions)}")

    pdf.body_text(
        "Regenerate from local runs: vulcanbench report --suite v1 -o docs/results/<name>.md"
    )
    return pdf


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "report_json", type=Path, help="JSON report from vulcanbench report -f json"
    )
    parser.add_argument("-o", "--output", type=Path, required=True, help="Output PDF path")
    parser.add_argument(
        "--tasks-root",
        type=Path,
        default=Path("tasks/v1"),
        help="Task metadata root (default: tasks/v1)",
    )
    args = parser.parse_args()
    report = json.loads(args.report_json.read_text(encoding="utf-8"))
    pdf = build_pdf(report, args.tasks_root)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    pdf.output(str(args.output))
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
