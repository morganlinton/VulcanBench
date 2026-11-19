"""Report amber snapshot entries that no collected test can produce.

Syrupy matches unused snapshots against test names with their parameters stripped, so an entry
whose parametrized id no longer exists stays hidden as long as any variant of that test is skipped.
"""

from __future__ import annotations

import re
from collections import defaultdict
from pathlib import Path

import pytest

_OPTION = "--check-orphan-snapshots"
_ENTRY_NAME = re.compile(r"^# name: (.+)$", re.MULTILINE)
_REPEAT_SUFFIX = re.compile(r"\.\d+$")
_collected_modules: set[Path] = set()
_skipped_modules: set[Path] = set()


def pytest_addoption(parser: pytest.Parser) -> None:
    """Register the orphan check option."""
    parser.addoption(
        _OPTION,
        action="store_true",
        help="Fail when an .ambr entry matches no collected test. Collect the whole suite, e.g. `pytest tests --collect-only`.",
    )


def pytest_collectreport(report: pytest.CollectReport) -> None:
    """Record the modules collected, and those skipped at import time such as the ones needing a missing extra."""
    if not report.nodeid.endswith(".py"):
        return
    module = Path(report.fspath).resolve()
    _collected_modules.add(module)
    if report.skipped:
        _skipped_modules.add(module)


@pytest.hookimpl(tryfirst=True)
def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    """Exit with a failure listing orphaned entries when the option is set."""
    if not config.getoption(_OPTION):
        return
    names: defaultdict[Path, set[str]] = defaultdict(set)
    for item in items:
        names[item.path.resolve()].add(".".join(item.nodeid.split("::")[1:]))
    orphans = [
        f"{ambr.relative_to(config.rootpath)}: {entry}"
        for ambr in sorted(config.rootpath.joinpath("tests").rglob("__snapshots__/*.ambr"))
        for entry in _orphans(ambr, names)
    ]
    if orphans:
        pytest.exit(f"{len(orphans)} orphaned snapshot entries:\n" + "\n".join(orphans), pytest.ExitCode.TESTS_FAILED)


def _orphans(ambr: Path, names: dict[Path, set[str]]) -> list[str]:
    module = ambr.parent.parent / f"{ambr.stem}.py"
    entries = _ENTRY_NAME.findall(ambr.read_text())
    if not module.exists():
        return entries
    if module not in _collected_modules or module in _skipped_modules:
        return []
    return [entry for entry in entries if _REPEAT_SUFFIX.sub("", entry) not in names[module]]
