"""A small stand-in for ``typer.testing.CliRunner`` for the argparse-based CLI.

``invoke(entry, args)`` calls ``entry(args)`` with stdout and stderr captured
into one ``output`` string (argparse writes usage errors to stderr), turns a
``SystemExit`` into ``exit_code`` and, like Typer's runner, reports an uncaught
exception as exit code 1 with the traceback in ``output`` and the exception on
``exception``.
"""

from __future__ import annotations

import contextlib
import io
import traceback
from collections.abc import Callable, Sequence
from dataclasses import dataclass


@dataclass
class Result:
    exit_code: int
    output: str
    exception: BaseException | None = None


class CliRunner:
    def invoke(self, entry: Callable[[list[str]], int], args: Sequence[str]) -> Result:
        buf = io.StringIO()
        exc: BaseException | None = None
        code = 0
        with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
            try:
                code = int(entry(list(args)) or 0)
            except SystemExit as e:
                exc = e
                if e.code is None:
                    code = 0
                elif isinstance(e.code, int):
                    code = e.code
                else:
                    buf.write(f"{e.code}\n")
                    code = 1
            except Exception as e:  # mirrors Typer's runner: a crash is exit code 1
                exc = e
                code = 1
                buf.write(traceback.format_exc())
        return Result(exit_code=code, output=buf.getvalue(), exception=exc)
