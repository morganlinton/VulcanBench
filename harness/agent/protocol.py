"""Tool protocol: the call and argument shapes the agent loop and executors share.

The argument models are plain dataclasses carrying the little validation the
loop relied on from Pydantic. Tool arguments arrive from a model's JSON, so
``from_args`` ignores unknown keys, reports missing required keys and wrong
types as ``ValueError`` (which ``ToolProtocol.execute`` turns into an error
observation for the model), and coerces scalars leniently: ``"120"`` becomes
``120`` and ``"true"`` becomes ``True``.
"""

from __future__ import annotations

import dataclasses
import types
import typing
from abc import ABC, abstractmethod
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any, ClassVar, Literal, Self

ToolName = Literal[
    "list_files",
    "read_file",
    "search_code",
    "edit_file",
    "run_command",
    "run_tests",
    "run_lint",
    "run_build",
    "security_scan",
]

_TRUE = frozenset({"true", "1", "yes", "y", "on", "t"})
_FALSE = frozenset({"false", "0", "no", "n", "off", "f"})
_HINTS: dict[type, dict[str, Any]] = {}


def _hints(cls: type) -> dict[str, Any]:
    """Resolved type hints per class (annotations are strings under PEP 563)."""
    hints = _HINTS.get(cls)
    if hints is None:
        hints = {
            name: hint
            for name, hint in typing.get_type_hints(cls).items()
            if typing.get_origin(hint) is not ClassVar
        }
        _HINTS[cls] = hints
    return hints


def _fields(cls: Any) -> tuple[dataclasses.Field[Any], ...]:
    return dataclasses.fields(cls)


def _asdict(obj: Any) -> dict[str, Any]:
    return dataclasses.asdict(obj)


def coerce(name: str, hint: Any, value: Any) -> Any:  # noqa: PLR0911, PLR0912
    """Coerce ``value`` to ``hint`` the way Pydantic's lax mode did for the few
    shapes tool arguments use: ``str``, ``int``, ``float``, ``bool``, ``Literal``,
    ``dict``, ``list`` and ``Optional`` of those. Other hints pass through."""
    origin = typing.get_origin(hint)
    if origin is types.UnionType or origin is typing.Union:
        members = typing.get_args(hint)
        if value is None and type(None) in members:
            return None
        concrete = [m for m in members if m is not type(None)]
        if len(concrete) == 1:
            return coerce(name, concrete[0], value)
        return value
    if origin is Literal:
        if value in typing.get_args(hint):
            return value
        raise ValueError(f"{name}: expected one of {list(typing.get_args(hint))}, got {value!r}")
    if hint is bool:
        if isinstance(value, bool):
            return value
        if isinstance(value, int) and value in (0, 1):
            return bool(value)
        if isinstance(value, str):
            lowered = value.strip().lower()
            if lowered in _TRUE:
                return True
            if lowered in _FALSE:
                return False
        raise ValueError(f"{name}: expected a boolean, got {value!r}")
    if hint is int:
        if isinstance(value, bool):
            return int(value)
        if isinstance(value, int):
            return value
        if isinstance(value, float) and value.is_integer():
            return int(value)
        if isinstance(value, str):
            try:
                return int(value.strip())
            except ValueError:
                pass
        raise ValueError(f"{name}: expected an integer, got {value!r}")
    if hint is float:
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            return float(value)
        if isinstance(value, str):
            try:
                return float(value.strip())
            except ValueError:
                pass
        raise ValueError(f"{name}: expected a number, got {value!r}")
    if hint is str:
        if isinstance(value, str):
            return value
        raise ValueError(f"{name}: expected a string, got {value!r}")
    if hint is dict or origin is dict:
        if isinstance(value, dict):
            return value
        raise ValueError(f"{name}: expected an object, got {value!r}")
    if hint is list or origin is list:
        if isinstance(value, list):
            return value
        raise ValueError(f"{name}: expected a list, got {value!r}")
    return value


class Model:
    """Base for the protocol dataclasses: validates and coerces on construction."""

    def __post_init__(self) -> None:
        for name, hint in _hints(type(self)).items():
            setattr(self, name, coerce(name, hint, getattr(self, name)))

    @classmethod
    def from_args(cls, raw: Any) -> Self:
        """Build from a model-supplied argument object.

        Unknown keys are dropped (Pydantic's default), a missing required key
        or a non-object payload is a ``ValueError``.
        """
        if raw is None:
            raw = {}
        if not isinstance(raw, Mapping):
            raise ValueError(
                f"{cls.__name__}: arguments must be an object, got {type(raw).__name__}"
            )
        fields = _fields(cls)
        names = {f.name for f in fields}
        known = {k: v for k, v in raw.items() if k in names}
        missing = [
            f.name
            for f in fields
            if f.name not in known
            and f.default is dataclasses.MISSING
            and f.default_factory is dataclasses.MISSING
        ]
        if missing:
            raise ValueError(f"{cls.__name__}: missing required argument(s): {', '.join(missing)}")
        return cls(**known)

    def to_dict(self) -> dict[str, Any]:
        return _asdict(self)


@dataclass(kw_only=True)
class ToolCall(Model):
    tool: ToolName
    args: dict[str, Any] = field(default_factory=dict)
    step: int = 0
    run_id: str = ""


@dataclass(kw_only=True)
class ToolObservation(Model):
    tool: ToolName
    result: Any
    error: str | None = None
    step: int = 0
    run_id: str = ""


@dataclass(kw_only=True)
class ListFilesArgs(Model):
    dir: str = "."
    recursive: bool = False


@dataclass(kw_only=True)
class ReadFileArgs(Model):
    path: str
    start_line: int | None = None
    limit: int | None = None


@dataclass(kw_only=True)
class SearchCodeArgs(Model):
    query: str
    glob: str | None = None
    semantic: bool = False
    timeout: int | None = 30


@dataclass(kw_only=True)
class EditFileArgs(Model):
    path: str
    old_string: str
    new_string: str


@dataclass(kw_only=True)
class RunCommandArgs(Model):
    cmd: str
    cwd: str | None = None
    timeout: int | None = 120


class ToolProtocol(ABC):
    @abstractmethod
    def list_files(self, args: ListFilesArgs) -> Any: ...

    @abstractmethod
    def read_file(self, args: ReadFileArgs) -> Any: ...

    @abstractmethod
    def search_code(self, args: SearchCodeArgs) -> Any: ...

    @abstractmethod
    def edit_file(self, args: EditFileArgs) -> Any: ...

    @abstractmethod
    def run_command(self, args: RunCommandArgs) -> Any: ...

    @abstractmethod
    def run_tests(self) -> Any: ...

    @abstractmethod
    def run_lint(self) -> Any: ...

    @abstractmethod
    def run_build(self) -> Any: ...

    @abstractmethod
    def security_scan(self, timeout_s: float | None = None) -> Any: ...

    def execute(self, call: ToolCall) -> ToolObservation:
        try:
            if call.tool == "list_files":
                res = self.list_files(ListFilesArgs.from_args(call.args))
            elif call.tool == "read_file":
                res = self.read_file(ReadFileArgs.from_args(call.args))
            elif call.tool == "search_code":
                res = self.search_code(SearchCodeArgs.from_args(call.args))
            elif call.tool == "edit_file":
                res = self.edit_file(EditFileArgs.from_args(call.args))
            elif call.tool == "run_command":
                res = self.run_command(RunCommandArgs.from_args(call.args))
            elif call.tool == "run_tests":
                res = self.run_tests()
            elif call.tool == "run_lint":
                res = self.run_lint()
            elif call.tool == "run_build":
                res = self.run_build()
            elif call.tool == "security_scan":
                res = self.security_scan()
            else:
                raise ValueError(f"unknown tool {call.tool}")
            return ToolObservation(tool=call.tool, result=res, step=call.step, run_id=call.run_id)
        except Exception as e:
            return ToolObservation(
                tool=call.tool, result=None, error=str(e), step=call.step, run_id=call.run_id
            )


def get_openai_tool_schemas() -> list[dict[str, Any]]:
    return [
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files in a directory",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "dir": {"type": "string", "default": "."},
                        "recursive": {"type": "boolean", "default": False},
                    },
                    "required": [],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read file contents, optionally with line range",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "start_line": {"type": "integer"},
                        "limit": {"type": "integer"},
                    },
                    "required": ["path"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "search_code",
                "description": "Search code with ripgrep or semantic (future)",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "glob": {"type": "string"},
                        "semantic": {"type": "boolean", "default": False},
                    },
                    "required": ["query"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Edit file with exact string match, returns unified diff",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "old_string": {"type": "string"},
                        "new_string": {"type": "string"},
                    },
                    "required": ["path", "old_string", "new_string"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run shell command, capture stdout/stderr/exit",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "cmd": {"type": "string"},
                        "cwd": {"type": "string"},
                        "timeout": {"type": "integer", "default": 120},
                    },
                    "required": ["cmd"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "run_tests",
                "description": "Run task-specific tests via verifier or script",
                "parameters": {"type": "object", "properties": {}},
            },
        },
    ]
