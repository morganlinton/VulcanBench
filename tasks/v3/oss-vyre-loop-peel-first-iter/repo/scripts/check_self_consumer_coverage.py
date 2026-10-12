from __future__ import annotations

from collections import defaultdict
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
PRIMITIVES = ROOT / "vyre-primitives" / "src"
SELF_CONSUMER_SURFACES = (
    ROOT / "vyre-libs" / "src" / "primitive_catalog.rs",
    ROOT / "vyre-driver" / "src" / "self_substrate",
)

CONST_RE = re.compile(
    r"(?:pub(?:\([^)]*\))?\s+)?const\s+"
    r"(?P<name>[A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*"
    r'"(?P<value>vyre-primitives::[^"]+)"\s*;'
)
OP_ENTRY_NEW_RE = re.compile(
    r"OpEntry::new\s*\(\s*(?P<arg>\"[^\"]+\"|[A-Za-z_][A-Za-z0-9_:]*)"
)
OP_ENTRY_ID_RE = re.compile(
    r"\bid\s*:\s*(?P<arg>\"[^\"]+\"|[A-Za-z_][A-Za-z0-9_:]*)"
)


def primitive_files() -> list[Path]:
    if not PRIMITIVES.is_dir():
        return []
    return sorted(PRIMITIVES.rglob("*.rs"))


def module_path_for(path: Path) -> str:
    rel = path.relative_to(PRIMITIVES).with_suffix("")
    parts = list(rel.parts)
    if parts[-1] == "mod":
        parts.pop()
    return "crate" if not parts else "crate::" + "::".join(parts)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def collect_constants(files: list[Path]) -> tuple[dict[Path, dict[str, str]], dict[str, list[str]]]:
    local: dict[Path, dict[str, str]] = {}
    global_by_name: dict[str, list[str]] = defaultdict(list)
    global_by_path: dict[str, list[str]] = defaultdict(list)
    for path in files:
        text = read_text(path)
        constants = {match.group("name"): match.group("value") for match in CONST_RE.finditer(text)}
        local[path] = constants
        module_path = module_path_for(path)
        for name, value in constants.items():
            global_by_name[name].append(value)
            global_by_path[f"{module_path}::{name}"].append(value)
    return local, {**global_by_name, **global_by_path}


def unique(values: list[str]) -> str | None:
    distinct = sorted(set(values))
    return distinct[0] if len(distinct) == 1 else None


def resolve_arg(arg: str, path: Path, local_constants: dict[Path, dict[str, str]], global_constants: dict[str, list[str]]) -> str | None:
    arg = arg.strip()
    if arg.startswith('"') and arg.endswith('"'):
        value = arg[1:-1]
        return value if value.startswith("vyre-primitives::") else None
    if arg in local_constants[path]:
        return local_constants[path][arg]
    if arg.startswith("crate::"):
        resolved = unique(global_constants.get(arg, []))
        if resolved is not None:
            return resolved
    tail = arg.rsplit("::", 1)[-1]
    if tail != "OP_ID":
        resolved = unique(global_constants.get(tail, []))
        if resolved is not None:
            return resolved
    return None


def collect_registered_ops(files: list[Path]) -> tuple[dict[str, set[str]], list[str]]:
    local_constants, global_constants = collect_constants(files)
    registered: dict[str, set[str]] = defaultdict(set)
    unresolved: list[str] = []
    for path in files:
        text = read_text(path)
        if "OpEntry" not in text:
            continue
        rel = str(path.relative_to(ROOT))
        for regex in (OP_ENTRY_NEW_RE, OP_ENTRY_ID_RE):
            for match in regex.finditer(text):
                arg = match.group("arg")
                op_id = resolve_arg(arg, path, local_constants, global_constants)
                if op_id is None:
                    if "inventory::submit!" in text:
                        unresolved.append(f"{rel}: {arg}")
                    continue
                if op_id.startswith("vyre-primitives::"):
                    registered[op_id].add(rel)
    return dict(registered), unresolved


def surface_files() -> list[Path]:
    files: list[Path] = []
    for surface in SELF_CONSUMER_SURFACES:
        if surface.is_file():
            files.append(surface)
        elif surface.is_dir():
            files.extend(sorted(surface.rglob("*.rs")))
    return files


def surface_text(files: list[Path]) -> str:
    return "\n".join(read_text(path) for path in files)


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "enforce"
    files = primitive_files()
    registered, unresolved = collect_registered_ops(files)
    surfaces = surface_files()
    combined_surface = surface_text(surfaces)

    if not registered:
        print(
            "self-consumer-coverage gate: no registered primitive harness ops found. "
            "Fix: repair primitive registry discovery.",
            file=sys.stderr,
        )
        return 1
    if not surfaces:
        print(
            "self-consumer-coverage gate: no self-consumer catalog surface found. "
            "Fix: restore vyre-libs/src/primitive_catalog.rs or an equivalent self-substrate surface.",
            file=sys.stderr,
        )
        return 1

    missing = sorted(op_id for op_id in registered if op_id not in combined_surface)
    covered = len(registered) - len(missing)

    if mode == "--report":
        print(f"self-consumer coverage: {covered}/{len(registered)} registered primitive ops covered")
        if unresolved:
            print("Unresolved registry ids:")
            for item in unresolved:
                print(f"  - {item}")
        if missing:
            print("Missing wrappers:")
            for op_id in missing:
                sources = ", ".join(sorted(registered[op_id]))
                print(f"  - {op_id} ({sources})")
        return 0

    if unresolved:
        print(
            f"self-consumer-coverage gate: {len(unresolved)} primitive registry ids could not be resolved.",
            file=sys.stderr,
        )
        for item in unresolved[:30]:
            print(f"  {item}", file=sys.stderr)
        if len(unresolved) > 30:
            print(f"  ... +{len(unresolved) - 30} more (run --report for full list)", file=sys.stderr)
        print("Fix: express primitive harness ids as literals or resolvable *_OP_ID constants.", file=sys.stderr)
        return 1

    if missing:
        print(
            f"self-consumer-coverage gate: {len(missing)} registered primitive ops lack self-consumer wrappers.",
            file=sys.stderr,
        )
        for op_id in missing[:40]:
            sources = ", ".join(sorted(registered[op_id]))
            print(f"  {op_id} ({sources})", file=sys.stderr)
        if len(missing) > 40:
            print(f"  ... +{len(missing) - 40} more (run --report for full list)", file=sys.stderr)
        print(
            "Fix: add each primitive to vyre-libs/src/primitive_catalog.rs so the primitive "
            "is exercised through the self-consumer catalog.",
            file=sys.stderr,
        )
        return 1

    print(
        f"self-consumer-coverage gate: {covered}/{len(registered)} registered primitive ops covered exactly."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
