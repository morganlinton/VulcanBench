"""CLI plumbing (stdlib only): plain-text output helpers and argparse glue.

The CLI used to render through Rich and parse through Typer. Output is now
plain text: the same words, no colour and no markup, so it reads identically
in a terminal, a log file, a CI capture or a test. The argparse helpers below
let each command stay a plain keyword-argument function.
"""

from __future__ import annotations

import argparse
import inspect
import sys
from collections.abc import Callable, Iterable, Sequence
from pathlib import Path


def echo(text: str = "") -> None:
    """Print one line to stdout (kept as a seam for tests and future styling)."""
    print(text)


def render_table(
    columns: Sequence[str],
    rows: Iterable[Sequence[object]],
    *,
    title: str | None = None,
) -> str:
    """Render ``rows`` under ``columns`` as an aligned, two-space-separated table.

    Cells may contain newlines; such a row is laid out over several physical
    lines. Every cell is rendered with ``str``. The header is underlined with
    dashes; an optional ``title`` goes on the first line.
    """
    cells = [[str(c) for c in row] for row in rows]
    n = len(columns)
    for row in cells:
        if len(row) != n:
            raise ValueError(f"row has {len(row)} cells, table has {n} columns: {row!r}")

    def width(text: str) -> int:
        return max((len(line) for line in text.split("\n")), default=0)

    widths = [len(h) for h in columns]
    for row in cells:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], width(cell))

    def fmt(parts: Sequence[str]) -> str:
        return "  ".join(p.ljust(w) for p, w in zip(parts, widths, strict=True)).rstrip()

    lines: list[str] = []
    if title:
        lines.append(title)
    lines.append(fmt(list(columns)))
    lines.append(fmt(["-" * w for w in widths]))
    for row in cells:
        split = [cell.split("\n") for cell in row]
        height = max(len(s) for s in split) if split else 1
        for k in range(height):
            lines.append(fmt([s[k] if k < len(s) else "" for s in split]))
    return "\n".join(lines)


def print_table(
    columns: Sequence[str],
    rows: Iterable[Sequence[object]],
    *,
    title: str | None = None,
) -> None:
    """``render_table`` straight to stdout."""
    print(render_table(columns, rows, title=title))


# --- argparse glue -----------------------------------------------------------


def existing_path(value: str) -> Path:
    """argparse ``type``: a path that must exist."""
    path = Path(value)
    if not path.exists():
        raise argparse.ArgumentTypeError(f"path does not exist: {value}")
    return path


def existing_dir(value: str) -> Path:
    """argparse ``type``: a directory that must exist."""
    path = existing_path(value)
    if not path.is_dir():
        raise argparse.ArgumentTypeError(f"not a directory: {value}")
    return path


def add_command(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
    name: str,
    handler: Callable[..., object],
) -> argparse.ArgumentParser:
    """Add ``name`` as a sub-command whose options are passed to ``handler`` by keyword.

    The handler's docstring is the command's help: its first paragraph in the
    command list, the whole text on ``<command> --help``.
    """
    doc = inspect.getdoc(handler) or ""
    summary = doc.split("\n\n", 1)[0].replace("\n", " ") if doc else None
    parser = subparsers.add_parser(
        name,
        help=_escape(summary) if summary else None,
        description=_escape(doc) if doc else None,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.set_defaults(_handler=handler)
    return parser


def add_group(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser], name: str, help: str
) -> argparse._SubParsersAction[argparse.ArgumentParser]:
    """Add ``name`` as a command group; returns the action to add its commands to.

    Invoking the group without a command prints the group's help.
    """
    group = subparsers.add_parser(name, help=help, description=help)
    group.set_defaults(_group=group)
    return group.add_subparsers(dest=f"_{name}_command", metavar="<command>")


def run_parser(parser: argparse.ArgumentParser, argv: Sequence[str] | None = None) -> int:
    """Parse ``argv`` (default ``sys.argv[1:]``) and run the selected command.

    No arguments prints the parser's help; a command group without a command
    prints that group's help. Commands report a non-zero exit by raising
    ``SystemExit``, which the console script propagates.
    """
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        parser.print_help()
        return 0
    ns = parser.parse_args(args)
    handler = getattr(ns, "_handler", None)
    if handler is None:
        group: argparse.ArgumentParser = getattr(ns, "_group", parser)
        group.print_help()
        return 0
    kwargs = {k: v for k, v in vars(ns).items() if not k.startswith("_")}
    handler(**kwargs)
    return 0


def _escape(text: str) -> str:
    """argparse formats help with ``%``; keep literal percent signs literal."""
    return text.replace("%", "%%")
