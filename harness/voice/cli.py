"""``vulcanbench voice`` sub-commands: render, run, report."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from harness.console import add_command, echo, run_parser
from harness.voice.audio import AudioCache, Condition
from harness.voice.items import load_items
from harness.voice.report import build_report, to_markdown
from harness.voice.runner import (
    DEFAULT_SEED,
    DEFAULT_SUBSET_N,
    JUDGE_DEFAULT,
    RunConfig,
    plan_units,
    run_suite,
    subset_ids,
)
from harness.voice.tts import DEFAULT_VOICES, PRIMARY_VOICE, get_tts

QUESTIONS = Path("tasks/voice-v1/questions.jsonl")
NOISE_DIR = Path("tasks/voice-v1/noise")
CACHE_DIR = Path("audio_cache/voice-v1")
OUT_DIR = Path("runs")

HELP = "Voice Eval Suite: text-vs-audio delta measurement"


def render(
    *,
    questions: Path = QUESTIONS,
    cache_dir: Path = CACHE_DIR,
    noise_dir: Path = NOISE_DIR,
    subset_n: int = DEFAULT_SUBSET_N,
    seed: int = DEFAULT_SEED,
    tts_provider: str = "openai",
) -> None:
    """Pre-render the audio cache (idempotent; reuses fresh files)."""
    items = load_items(questions)
    cache = AudioCache(cache_dir, noise_dir=noise_dir)
    tts = get_tts(tts_provider)
    sub = set(subset_ids(items, subset_n, seed))
    rendered = reused = 0
    conditions_full = [Condition(v, "normal", "clean") for v in DEFAULT_VOICES]
    conditions_sub = [
        Condition(PRIMARY_VOICE, "fast", "clean"),
        Condition(PRIMARY_VOICE, "normal", "noise10db"),
    ]
    for it in items:
        conds = list(conditions_full) + (conditions_sub if it.id in sub else [])
        for cond in conds:
            if cache.is_fresh(it, cond):
                reused += 1
            else:
                cache.ensure(it, cond, tts)
                rendered += 1
    echo(f"rendered {rendered}, reused {reused} → {cache_dir}")


def run(
    *,
    models: str,
    questions: Path = QUESTIONS,
    out_dir: Path = OUT_DIR,
    cache_dir: Path = CACHE_DIR,
    noise_dir: Path = NOISE_DIR,
    judge_model: str = JUDGE_DEFAULT,
    subset_n: int = DEFAULT_SUBSET_N,
    seed: int = DEFAULT_SEED,
    dry_run: bool = False,
    run_id: str | None = None,
) -> None:
    """Run the suite (resumable: re-invoke with --run-id to skip finished units)."""
    cfg = RunConfig(
        models=[m.strip() for m in models.split(",") if m.strip()],
        questions_path=questions,
        out_dir=out_dir,
        cache_dir=cache_dir,
        noise_dir=noise_dir,
        subset_n=subset_n,
        seed=seed,
        dry_run=dry_run,
        judge_model=judge_model,
    )
    if run_id:
        cfg.run_id = run_id
    units = plan_units(cfg, load_items(questions))
    echo(
        f"run {cfg.run_id}: {len(units)} planned units "
        f"({'dry-run, ' if dry_run else ''}judge {judge_model})"
    )
    run_dir = run_suite(cfg)
    echo(f"done → {run_dir}/results.jsonl")


def report(*, run_dir: Path, output: Path | None = None, as_json: bool = False) -> None:
    """Aggregate a run into voice-tax tables (markdown by default)."""
    data = build_report(run_dir)
    rendered = json.dumps(data, indent=1) if as_json else to_markdown(data)
    if output:
        output.write_text(rendered)
        echo(f"wrote {output}")
    else:
        echo(rendered)


def register_voice_commands(
    commands: argparse._SubParsersAction[argparse.ArgumentParser],
) -> None:
    """Attach ``render``, ``run`` and ``report`` to a sub-parser action."""
    p = add_command(commands, "render", render)
    p.add_argument("--questions", dest="questions", type=Path, default=QUESTIONS)
    p.add_argument("--cache-dir", dest="cache_dir", type=Path, default=CACHE_DIR)
    p.add_argument("--noise-dir", dest="noise_dir", type=Path, default=NOISE_DIR)
    p.add_argument("--subset-n", dest="subset_n", type=int, default=DEFAULT_SUBSET_N)
    p.add_argument("--seed", dest="seed", type=int, default=DEFAULT_SEED)
    p.add_argument("--tts", dest="tts_provider", default="openai")

    p = add_command(commands, "run", run)
    p.add_argument(
        "--models",
        "-m",
        dest="models",
        required=True,
        help="Comma-separated adapters, e.g. openai-realtime,gemini-live,qwen-omni "
        "(optionally adapter:model)",
    )
    p.add_argument("--questions", dest="questions", type=Path, default=QUESTIONS)
    p.add_argument("--output-dir", "-o", dest="out_dir", type=Path, default=OUT_DIR)
    p.add_argument("--cache-dir", dest="cache_dir", type=Path, default=CACHE_DIR)
    p.add_argument("--noise-dir", dest="noise_dir", type=Path, default=NOISE_DIR)
    p.add_argument("--judge-model", dest="judge_model", default=JUDGE_DEFAULT)
    p.add_argument("--subset-n", dest="subset_n", type=int, default=DEFAULT_SUBSET_N)
    p.add_argument("--seed", dest="seed", type=int, default=DEFAULT_SEED)
    p.add_argument(
        "--dry-run",
        dest="dry_run",
        action="store_true",
        help="First 5 items, one voice, clean only",
    )
    p.add_argument("--run-id", dest="run_id", default=None, help="Resume an existing run directory")

    p = add_command(commands, "report", report)
    p.add_argument("run_dir", type=Path, help="runs/voice-<id> directory")
    p.add_argument("--output", "-o", dest="output", type=Path, default=None)
    p.add_argument("--json", dest="as_json", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    """A standalone ``vulcanbench voice`` parser (the main CLI nests the same commands)."""
    parser = argparse.ArgumentParser(prog="vulcanbench voice", description=HELP)
    register_voice_commands(parser.add_subparsers(dest="_command", metavar="<command>"))
    return parser


def voice_main(argv: list[str] | None = None) -> int:
    """Entry point for running the voice commands on their own (used by tests)."""
    return run_parser(build_parser(), argv)
