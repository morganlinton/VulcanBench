"""Hidden pass-to-pass guards: explicit names, start_soon naming, and
create_task results are unchanged."""

from __future__ import annotations

import anyio


async def taskfunc() -> int:
    await anyio.sleep(0)
    return 7


def test_explicit_name_still_wins() -> None:
    names: dict[str, str] = {}

    async def main() -> None:
        async with anyio.create_task_group() as tg:
            handle = tg.create_task(taskfunc(), name="custom-name")
            names["n"] = handle.name
            await handle

    anyio.run(main)
    assert names["n"] == "custom-name"


def test_start_soon_name_already_module_qualified() -> None:
    seen: dict[str, str] = {}

    async def observer() -> None:
        seen["name"] = anyio.get_current_task().name or ""

    async def main() -> None:
        async with anyio.create_task_group() as tg:
            tg.start_soon(observer)

    anyio.run(main)
    assert seen["name"].startswith(f"{__name__}.")


def test_create_task_result_awaitable() -> None:
    out: dict[str, int] = {}

    async def main() -> None:
        async with anyio.create_task_group() as tg:
            handle = tg.create_task(taskfunc())
            out["v"] = await handle

    anyio.run(main)
    assert out["v"] == 7


def test_trio_backend_runs() -> None:
    async def main() -> None:
        await anyio.sleep(0)

    anyio.run(main, backend="trio")
