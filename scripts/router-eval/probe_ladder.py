#!/usr/bin/env python3
"""Single-request routing probe for model routers.

For every task in a suite (default ``tasks/router-v1``) build ONE chat prompt
(the issue, a file tree, and the files the gold patch touches, trimmed to a
token budget per repo scale) and send it to each router n times with no
tools, asking for a short plan. The response's ``model`` is what the router
served; ``usage.cost`` (when the gateway returns it) is what it charged.

A control set repeats the small+easy prompts padded with files from a large
repo of the same language up to the large budget. If the served-model mix
moves with padding but not with the cell, the router routes on length.

Usage:
    python scripts/router-eval/probe_ladder.py \\
        --router openrouter=https://openrouter.ai/api/v1=openrouter/auto \\
        --router martian=https://api.withmartian.com/v1=auto \\
        --n 8 --out ladder.jsonl
    python scripts/router-eval/probe_ladder.py --dry-run     # prompts + sizes only

Keys: ``OPENROUTER_API_KEY`` for the router named ``openrouter``, otherwise
``ROUTER_<NAME>_API_KEY``. A router given as ``name=MODEL`` (no base URL)
reads ``ROUTER_<NAME>_BASE_URL``. Rows already in ``--out`` are skipped, so a
stopped run resumes.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from harness.tasks import load_task  # noqa: E402

SYSTEM = (
    "You are a senior engineer triaging a repository issue. Read the issue and the "
    "code excerpts, then answer with a plan of at most 200 words: which files change, "
    "what the fix is, and how you would test it. Do not write the full patch."
)
CHARS_PER_TOKEN = 4
SOURCE_SUFFIXES = {
    ".py",
    ".rs",
    ".toml",
    ".md",
    ".txt",
    ".cfg",
    ".ini",
    ".json",
    ".yaml",
    ".yml",
    ".go",
    ".ts",
    ".js",
    ".c",
    ".h",
    ".cc",
    ".cpp",
    ".java",
}
SKIP_DIRS = {".git", "vendor", "target", "node_modules", "__pycache__", ".venv", "build", "dist"}
PATCH_FILE = re.compile(r"^\+\+\+ (?:b/)?(.+)$", re.MULTILINE)


@dataclass
class RouterSpec:
    name: str
    base_url: str
    model: str
    api_key_env: str
    api: str = "chat"  # "chat" (/chat/completions) or "responses" (/responses)

    @classmethod
    def parse(cls, text: str) -> RouterSpec:
        parts = text.split("=")
        if len(parts) == 3:
            name, base, model = parts
        elif len(parts) == 2:
            name, model = parts
            base = os.environ.get(f"ROUTER_{_slug(name)}_BASE_URL", "")
            if not base:
                raise SystemExit(
                    f"router {name!r}: give name=BASE_URL=MODEL or set ROUTER_{_slug(name)}_BASE_URL"
                )
        else:
            raise SystemExit(f"router spec must be name=BASE_URL=MODEL, got {text!r}")
        key_env = "OPENROUTER_API_KEY" if name == "openrouter" else f"ROUTER_{_slug(name)}_API_KEY"
        api = os.environ.get(f"ROUTER_{_slug(name)}_API", "chat").strip().lower() or "chat"
        if api not in {"chat", "responses"}:
            raise SystemExit(f"ROUTER_{_slug(name)}_API must be 'chat' or 'responses', got {api!r}")
        return cls(name=name, base_url=base.rstrip("/"), model=model, api_key_env=key_env, api=api)


def _slug(name: str) -> str:
    return name.upper().replace("-", "_")


@dataclass
class Prompt:
    task_id: str
    cell: dict[str, str]
    control: str | None
    text: str
    budget_tokens: int
    files: list[str] = field(default_factory=list)

    @property
    def approx_tokens(self) -> int:
        return len(self.text) // CHARS_PER_TOKEN


def approx_tokens(text: str) -> int:
    return len(text) // CHARS_PER_TOKEN


def _repo_root(task_root: Path, scratch: Path) -> Path:
    repo = task_root / "repo"
    if repo.is_dir():
        return repo
    snap = task_root / "repo_snapshot.tar.gz"
    if snap.exists():
        out = scratch / task_root.name
        if not out.exists():
            out.mkdir(parents=True)
            with tarfile.open(snap) as tf:
                tf.extractall(out, filter="data")
        inner = [p for p in out.iterdir() if p.is_dir()]
        return inner[0] if len(inner) == 1 and not (out / "Cargo.toml").exists() else out
    raise SystemExit(f"{task_root}: no repo/ or repo_snapshot.tar.gz")


def _source_files(repo: Path) -> list[Path]:
    files: list[Path] = []
    for p in sorted(repo.rglob("*")):
        if any(part in SKIP_DIRS for part in p.relative_to(repo).parts):
            continue
        if p.is_file() and p.suffix in SOURCE_SUFFIXES:
            files.append(p)
    return files


def _file_tree(repo: Path, limit: int = 120) -> str:
    rels = [str(p.relative_to(repo)) for p in _source_files(repo)]
    shown = rels[:limit]
    more = f"\n... and {len(rels) - limit} more files" if len(rels) > limit else ""
    return "\n".join(shown) + more


def _gold_files(task_root: Path) -> list[str]:
    patch = task_root / "gold_patch.diff"
    if not patch.exists():
        return []
    return [m for m in PATCH_FILE.findall(patch.read_text(errors="replace")) if m != "/dev/null"]


def _fit(parts: list[tuple[str, str]], budget_chars: int) -> tuple[str, list[str]]:
    """Concatenate ``(label, body)`` blocks, truncating the last to fit."""
    out: list[str] = []
    used = 0
    names: list[str] = []
    for label, body in parts:
        block = f"\n\n### {label}\n```\n{body}\n```"
        if used + len(block) > budget_chars:
            room = budget_chars - used - len(label) - 40
            if room > 200:
                out.append(f"\n\n### {label} (truncated)\n```\n{body[:room]}\n```")
                names.append(label)
            break
        out.append(block)
        names.append(label)
        used += len(block)
    return "".join(out), names


def build_prompt(
    task_root: Path,
    cell: dict[str, str],
    budget_tokens: int,
    scratch: Path,
    pad_from: Path | None = None,
    control: str | None = None,
) -> Prompt:
    task = load_task(task_root.name, task_root.parent)
    repo = _repo_root(task_root, scratch)
    head = f"# Issue\n\n{task.issue.strip()}\n\n# Repository files\n\n{_file_tree(repo)}"
    budget_chars = budget_tokens * CHARS_PER_TOKEN
    blocks: list[tuple[str, str]] = []
    for rel in _gold_files(task_root):
        p = repo / rel
        if p.is_file():
            blocks.append((rel, p.read_text(errors="replace")))
    if not blocks:
        for p in _source_files(repo)[:6]:
            blocks.append((str(p.relative_to(repo)), p.read_text(errors="replace")))
    body, names = _fit(blocks, max(budget_chars - len(head), 400))
    text = head + body
    if pad_from is not None:
        pad_repo = _repo_root(pad_from, scratch)
        pad_blocks = [
            (f"context/{p.relative_to(pad_repo)}", p.read_text(errors="replace"))
            for p in _source_files(pad_repo)
        ]
        pad_text, pad_names = _fit(pad_blocks, max(budget_chars - len(text), 0))
        text += "\n\n# Additional repository context (unrelated modules)" + pad_text
        names += pad_names
    return Prompt(
        task_id=task_root.name,
        cell=dict(cell),
        control=control,
        text=text,
        budget_tokens=budget_tokens,
        files=names,
    )


def load_suite(suite_dir: Path) -> tuple[list[str], dict[str, dict[str, str]]]:
    suite = json.loads((suite_dir / "suite.json").read_text())
    tasks = [t for t in suite.get("tasks", []) if (suite_dir / t / "metadata.json").exists()]
    cells: dict[str, dict[str, str]] = {}
    for tid in tasks:
        meta = json.loads((suite_dir / tid / "metadata.json").read_text())
        cell = meta.get("router_cell") or suite.get("cells", {}).get(tid)
        if not cell:
            raise SystemExit(f"{tid}: no router_cell in metadata.json or suite.json")
        cells[tid] = cell
    return tasks, cells


def build_all(
    suite_dir: Path,
    small_budget: int,
    large_budget: int,
    scratch: Path,
    with_control: bool = True,
) -> list[Prompt]:
    tasks, cells = load_suite(suite_dir)
    prompts: list[Prompt] = []
    for tid in tasks:
        cell = cells[tid]
        budget = small_budget if cell["scale"] == "small" else large_budget
        prompts.append(build_prompt(suite_dir / tid, cell, budget, scratch))
    if with_control:
        for tid in tasks:
            cell = cells[tid]
            if cell["scale"] != "small" or cell["difficulty"] != "easy":
                continue
            donor = next(
                (
                    d
                    for d in tasks
                    if cells[d]["language"] == cell["language"]
                    and cells[d]["scale"] == "large"
                    and cells[d]["difficulty"] == "easy"
                ),
                None,
            )
            prompts.append(
                build_prompt(
                    suite_dir / tid,
                    cell,
                    large_budget,
                    scratch,
                    pad_from=(suite_dir / donor) if donor else None,
                    control="padded",
                )
            )
    return prompts


def call_router(router: RouterSpec, prompt: Prompt, timeout: float = 180.0) -> dict[str, Any]:
    key = os.environ.get(router.api_key_env)
    if not key:
        raise SystemExit(f"{router.api_key_env} is not set")
    if router.api == "responses":
        # Ramp Router bills (``usage.cost``) only on this surface.
        payload: dict[str, Any] = {
            "model": router.model,
            "instructions": SYSTEM,
            "input": prompt.text,
            "max_output_tokens": 400,
        }
        path = "/responses"
    else:
        payload = {
            "model": router.model,
            "messages": [
                {"role": "system", "content": SYSTEM},
                {"role": "user", "content": prompt.text},
            ],
            "max_tokens": 400,
            "usage": {"include": True},
        }
        path = "/chat/completions"
    req = urllib.request.Request(
        f"{router.base_url}{path}",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        method="POST",
    )
    started = time.monotonic()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", errors="replace")[:300]
        return {
            "error": f"HTTP {e.code}: {detail}",
            "latency_s": round(time.monotonic() - started, 3),
        }
    except (urllib.error.URLError, TimeoutError) as e:
        return {"error": str(e), "latency_s": round(time.monotonic() - started, 3)}
    row = parse_body(router.api, body)
    row["latency_s"] = round(time.monotonic() - started, 3)
    return row


def parse_body(api: str, body: dict[str, Any]) -> dict[str, Any]:
    """The ladder row fields from a chat-completions or Responses body."""
    usage = body.get("usage") or {}
    common = {
        "served_model": body.get("model"),
        "response_id": body.get("id"),
        "provider": body.get("provider"),
        "reported_cost_usd": usage.get("cost"),
    }
    if api == "responses":
        text = "".join(
            block.get("text", "")
            for item in body.get("output") or []
            if item.get("type") == "message"
            for block in item.get("content") or []
            if block.get("type") in {"output_text", "text"}
        ) or str(body.get("output_text") or "")
        incomplete = body.get("incomplete_details") or {}
        return {
            **common,
            "prompt_tokens": usage.get("input_tokens"),
            "completion_tokens": usage.get("output_tokens"),
            "finish_reason": incomplete.get("reason") or body.get("status"),
            "answer_chars": len(text),
        }
    choice = (body.get("choices") or [{}])[0]
    return {
        **common,
        "prompt_tokens": usage.get("prompt_tokens"),
        "completion_tokens": usage.get("completion_tokens"),
        "finish_reason": choice.get("finish_reason"),
        "answer_chars": len((choice.get("message") or {}).get("content") or ""),
    }


def _row_key(row: dict[str, Any]) -> tuple[str, str, str, int]:
    return (row["router"], row["task_id"], row.get("control") or "", int(row["rep"]))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--suite", default=str(ROOT / "tasks" / "router-v1"))
    ap.add_argument(
        "--router", action="append", default=[], help="name=BASE_URL=MODEL (repeatable)"
    )
    ap.add_argument("--n", type=int, default=8, help="requests per prompt per router")
    ap.add_argument("--out", default="ladder.jsonl")
    ap.add_argument("--small-budget", type=int, default=6000, help="prompt tokens for small repos")
    ap.add_argument("--large-budget", type=int, default=24000, help="prompt tokens for large repos")
    ap.add_argument("--no-control", action="store_true", help="skip the padded control prompts")
    ap.add_argument("--dry-run", action="store_true", help="print prompt sizes, call nothing")
    ap.add_argument("--print-prompt", help="task id whose prompt to print in --dry-run")
    args = ap.parse_args(argv)

    suite_dir = Path(args.suite)
    with tempfile.TemporaryDirectory(prefix="router-ladder-") as tmp:
        prompts = build_all(
            suite_dir, args.small_budget, args.large_budget, Path(tmp), not args.no_control
        )
        if args.dry_run:
            for p in prompts:
                tag = f" [{p.control}]" if p.control else ""
                cell = "/".join(p.cell[k] for k in ("language", "scale", "difficulty"))
                print(
                    f"{p.task_id:45} {cell:22}{tag:10} ~{p.approx_tokens:>6} tok  files={len(p.files)}"
                )
                if args.print_prompt == p.task_id:
                    print(p.text)
            routers = [RouterSpec.parse(r) for r in args.router]
            total = len(prompts) * args.n * max(len(routers), 1)
            print(
                f"\n{len(prompts)} prompts x {args.n} reps x {max(len(routers), 1)} routers = {total} requests"
            )
            return 0

        routers = [RouterSpec.parse(r) for r in args.router]
        if not routers:
            ap.error("give at least one --router (or --dry-run)")
        out = Path(args.out)
        done: set[tuple[str, str, str, int]] = set()
        if out.exists():
            for line in out.read_text().splitlines():
                if line.strip():
                    done.add(_row_key(json.loads(line)))
        with out.open("a", encoding="utf-8") as fh:
            for router in routers:
                for p in prompts:
                    for rep in range(args.n):
                        key = (router.name, p.task_id, p.control or "", rep)
                        if key in done:
                            continue
                        result = call_router(router, p)
                        row = {
                            "router": router.name,
                            "requested_model": router.model,
                            "task_id": p.task_id,
                            "cell": p.cell,
                            "control": p.control,
                            "rep": rep,
                            "budget_tokens": p.budget_tokens,
                            "approx_prompt_tokens": p.approx_tokens,
                            "ts": time.time(),
                            **result,
                        }
                        fh.write(json.dumps(row) + "\n")
                        fh.flush()
                        status = result.get("served_model") or result.get("error", "?")
                        print(
                            f"{router.name:12} {p.task_id:45} {p.control or '':7} rep={rep} -> {status}"
                        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
