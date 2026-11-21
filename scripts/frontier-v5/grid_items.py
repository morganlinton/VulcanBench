#!/usr/bin/env python3
"""Read a Frontier v5 grid trial's per-item result and check it against its reward.

usage: scripts/frontier-v5/grid_items.py <trial-dir> [...]

A grid environment's verifier writes ``items.json`` next to ``reward.json``
in the trial's verifier directory (schema agreed in the grid brief,
2026-11-21):

    {"schema": 1,
     "tamper_clean": true,
     "items": {"<item-id>": {"kind": "defect|feature|perf|diagnosis",
                              "done": true,
                              "checks_passed": 3, "checks_total": 3}},
     "guards": {"total": 120, "failed": ["<check id>", ...]}}

and ``reward.json`` carries ``reward`` (items done over items total, a
float), ``items_done`` and ``items_total``.

Rules (PLAN-v6 section 5): an item is done only when all of its held-out
checks pass, so ``done`` with ``checks_passed < checks_total`` is malformed.
The converse is not enforced: how a broken guard is charged (to its item,
or one item off the environment) is the pilot's call, and the verifier
applies it by clearing ``done``. A trial whose tamper scan fails scores 0
items whatever ``items`` says. The reward must equal the graded items over
the total (to 1e-9), and ``items_done`` and ``items_total`` must match;
otherwise the trial is reported **inconsistent** and the board refuses it.

The CLI prints one line per trial and exits 1 if any trial is malformed or
inconsistent.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

SCHEMA = 1
KINDS = ("defect", "feature", "perf", "diagnosis")
ITEM_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
TOLERANCE = 1e-9


class ItemsError(ValueError):
    """items.json does not follow the schema."""


def _int(v: Any) -> int | None:
    """v when it is a plain integer (not a bool), else None."""
    return v if isinstance(v, int) and not isinstance(v, bool) else None


def _parse_item(iid: Any, it: Any) -> dict[str, Any]:
    if not isinstance(iid, str) or not ITEM_ID.match(iid):
        raise ItemsError(f"item id {iid!r} is not short kebab-case")
    if not isinstance(it, dict):
        raise ItemsError(f"{iid}: expected an object")
    if it.get("kind") not in KINDS:
        raise ItemsError(f"{iid}: kind must be one of {', '.join(KINDS)}")
    if not isinstance(it.get("done"), bool):
        raise ItemsError(f"{iid}: done must be true or false")
    passed, total = _int(it.get("checks_passed")), _int(it.get("checks_total"))
    if passed is None or total is None or total < 1 or not 0 <= passed <= total:
        raise ItemsError(f"{iid}: need 0 <= checks_passed <= checks_total and checks_total >= 1")
    if it["done"] and passed != total:
        raise ItemsError(f"{iid}: done with {passed} of {total} held-out checks passing")
    return {"kind": it["kind"], "done": it["done"], "checks_passed": passed, "checks_total": total}


def _parse_guards(guards: Any) -> dict[str, Any]:
    if not isinstance(guards, dict):
        raise ItemsError("guards must be an object")
    gtotal, failed = _int(guards.get("total")), guards.get("failed")
    if gtotal is None or gtotal < 0:
        raise ItemsError("guards.total must be a non-negative integer")
    if not isinstance(failed, list) or not all(isinstance(x, str) for x in failed):
        raise ItemsError("guards.failed must be a list of check ids")
    if len(set(failed)) > gtotal:
        raise ItemsError(f"guards.failed lists {len(set(failed))} ids but guards.total is {gtotal}")
    return {"total": gtotal, "failed": sorted(set(failed))}


def parse_items(doc: Any) -> dict[str, Any]:
    """Validate an items.json document and return it normalised."""
    if not isinstance(doc, dict):
        raise ItemsError("items.json is not an object")
    if doc.get("schema") != SCHEMA:
        raise ItemsError(f"schema must be {SCHEMA} (got {doc.get('schema')!r})")
    if not isinstance(doc.get("tamper_clean"), bool):
        raise ItemsError("tamper_clean must be true or false")
    items = doc.get("items")
    if not isinstance(items, dict) or not items:
        raise ItemsError("items must be a non-empty object")
    return {
        "schema": SCHEMA,
        "tamper_clean": doc["tamper_clean"],
        "items": {iid: _parse_item(iid, it) for iid, it in items.items()},
        "guards": _parse_guards(doc.get("guards")),
    }


@dataclass
class TrialItems:
    trial: str
    items_total: int = 0
    items_done: int = 0
    items_done_before_tamper: int = 0
    tamper_clean: bool | None = None
    guards_total: int = 0
    guards_failed: list[str] = field(default_factory=list)
    by_kind: dict[str, list[int]] = field(default_factory=dict)
    items: dict[str, dict[str, Any]] = field(default_factory=dict)
    reward: float | None = None
    consistent: bool = False
    problems: list[str] = field(default_factory=list)

    def summary(self) -> dict[str, Any]:
        d = asdict(self)
        d.pop("items")
        return d


def _load(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError) as exc:
        raise ItemsError(f"{path.name}: {exc}") from exc


def read_trial(trial: Path) -> TrialItems | None:
    """Score one trial; None when its verifier wrote no reward (not graded)."""
    vdir = trial / "verifier"
    if not (vdir / "reward.json").is_file():
        return None
    t = TrialItems(trial=trial.name)
    if not (vdir / "items.json").is_file():
        t.problems.append("the verifier wrote reward.json but no items.json")
        return t
    try:
        doc = parse_items(_load(vdir / "items.json"))
        reward_doc = _load(vdir / "reward.json")
    except ItemsError as exc:
        t.problems.append(f"malformed: {exc}")
        return t
    items = doc["items"]
    t.items = items
    t.items_total = len(items)
    t.items_done_before_tamper = sum(1 for it in items.values() if it["done"])
    t.tamper_clean = doc["tamper_clean"]
    t.items_done = t.items_done_before_tamper if t.tamper_clean else 0
    t.guards_total = doc["guards"]["total"]
    t.guards_failed = doc["guards"]["failed"]
    for it in items.values():
        k = t.by_kind.setdefault(it["kind"], [0, 0])
        k[1] += 1
        k[0] += int(it["done"] and bool(t.tamper_clean))
    t.by_kind = dict(sorted(t.by_kind.items()))
    reward = reward_doc.get("reward") if isinstance(reward_doc, dict) else None
    if isinstance(reward, bool) or not isinstance(reward, (int, float)):
        t.problems.append("reward.json has no numeric reward")
        return t
    t.reward = float(reward)
    expect = t.items_done / t.items_total
    if abs(t.reward - expect) > TOLERANCE:
        t.problems.append(f"reward {t.reward} is not {t.items_done}/{t.items_total}")
    for key, want in (("items_done", t.items_done), ("items_total", t.items_total)):
        if reward_doc.get(key) != want:
            t.problems.append(f"reward.json {key}={reward_doc.get(key)!r}, items.json gives {want}")
    t.consistent = not t.problems
    return t


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("trials", nargs="+", type=Path)
    args = ap.parse_args(argv)
    bad = 0
    for trial in args.trials:
        t = read_trial(trial)
        if t is None:
            print(f"{trial.name}: not graded (no verifier/reward.json)")
            continue
        state = "ok" if t.consistent else "INCONSISTENT"
        bad += not t.consistent
        print(
            f"{trial.name}: {t.items_done}/{t.items_total} items, tamper_clean={t.tamper_clean}, "
            f"guards failed {len(t.guards_failed)}/{t.guards_total}, reward {t.reward} {state}"
            + (f" ({'; '.join(t.problems)})" if t.problems else "")
        )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
