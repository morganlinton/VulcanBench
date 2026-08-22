"""Hidden fail-to-pass tests: TaskGroup.create_task default task names must
include the module name, matching start_soon's naming convention, on both
backends. Graded via the public anyio API and TaskHandle.name."""

from __future__ import annotations

import anyio


async def taskfunc() -> int:
    await anyio.sleep(0)
    return 42


EXPECTED = f"{__name__}.{taskfunc.__qualname__}"


def _default_name(backend: str) -> str:
    result: dict[str, str] = {}

    async def main() -> None:
        async with anyio.create_task_group() as tg:
            handle = tg.create_task(taskfunc())
            result["name"] = handle.name
            await handle

    anyio.run(main, backend=backend)
    return result["name"]


def test_asyncio_default_name_includes_module() -> None:
    assert _default_name("asyncio") == EXPECTED


def test_trio_default_name_includes_module() -> None:
    assert _default_name("trio") == EXPECTED


def test_matches_start_soon_convention() -> None:
    # start_soon's default name for the same function is module-qualified;
    # create_task must produce the same name.
    seen: dict[str, str] = {}

    async def observer() -> None:
        seen["start_soon"] = anyio.get_current_task().name or ""

    async def main() -> None:
        async with anyio.create_task_group() as tg:
            tg.start_soon(observer)
        async with anyio.create_task_group() as tg:
            handle = tg.create_task(taskfunc())
            seen["create_task"] = handle.name
            await handle

    anyio.run(main)
    assert seen["create_task"] == f"{__name__}.{observer.__qualname__}".replace(
        observer.__qualname__, taskfunc.__qualname__
    )
    assert seen["start_soon"].startswith(f"{__name__}.")
