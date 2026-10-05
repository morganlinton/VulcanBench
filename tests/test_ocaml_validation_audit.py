"""Faulty-control audits must distinguish behavioral failure from bad code."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/validate_ocaml_pilot.py"
SPEC = importlib.util.spec_from_file_location("ocaml_validation_audit", SCRIPT)
assert SPEC and SPEC.loader
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


@pytest.mark.parametrize(
    "output",
    [
        'File "typing.ml", line 16:\nError: Unbound module Types\n',
        'File "cache.ml", line 3:\nError (warning 26 [unused-var]): unused variable\n',
        "\x1b[31mError:\x1b[0m This expression has type string but int was expected",
        'RuntimeError: File "client.ml", line 2:\nError: Unbound value f\n',
    ],
)
def test_rejects_compiler_diagnostics(output: str) -> None:
    assert audit.contains_ocaml_compile_error(output)


@pytest.mark.parametrize(
    "output",
    [
        "AssertionError: missing int field 0\n(field_imm 0 x)\n",
        'AssertionError: Fatal error: exception Failure("shared record root")\n',
        'Fatal error: exception Assert_failure("checks.ml", 12, 3)\n',
        "  OCAMLOPT typing/errortrace.cmx\nPASS public_interfaces\n",
    ],
)
def test_accepts_compiling_behavioral_failures(output: str) -> None:
    assert not audit.contains_ocaml_compile_error(output)


def test_local_exercise_uses_host_tools_and_keeps_hidden_tests_out_of_setup(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class NoDocker:
        def __init__(self, *args: object, **kwargs: object) -> None:
            raise AssertionError("local validation must not start Docker")

    monkeypatch.setattr(audit, "DockerToolExecutor", NoDocker)
    root = tmp_path / "tasks/local-task"
    (root / "repo").mkdir(parents=True)
    (root / "tests/hidden").mkdir(parents=True)
    (root / "repo/value").write_text("public\n")
    (root / "tests/hidden/check.py").write_text(
        "from pathlib import Path\nassert Path('value').read_text() == 'public\\n'\n"
    )
    (root / "issue.md").write_text("Keep the public value.\n")
    (root / "metadata.json").write_text(
        json.dumps(
            {
                "test_timeout_s": 30,
                "setup": [
                    {
                        "name": "private tests absent",
                        "cmd": "python -c \"from pathlib import Path; assert not Path('hidden').exists()\"",
                    }
                ],
                "tests": {
                    "fail_to_pass": [{"name": "value", "cmd": "python hidden/check.py"}],
                    "pass_to_pass": [],
                },
            }
        )
    )
    result = audit.exercise(root, None, sandbox="local")
    assert result["sandbox"] == "local"
    assert result["verdict"]["scores"]["functional"] == 1
    assert result["setup_commands"][0]["exit_code"] == 0
    assert result["commands"][0]["exit_code"] == 0
    assert len(result["task_hash"]) == 64


def test_unknown_sandbox_is_rejected_before_preparing_workspaces(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="sandbox must"):
        audit.exercise(tmp_path / "absent-task", None, sandbox="auto")
