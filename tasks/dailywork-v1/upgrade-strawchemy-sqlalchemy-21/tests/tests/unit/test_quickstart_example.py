from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from quickstart.app import create_app
from quickstart.models import Base, Post, User
from quickstart.schema import schema
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession


@dataclass
class _Context:
    session: AsyncSession


def test_asgi_app_builds() -> None:
    """The Litestar app the Getting started page ends on can be constructed."""
    create_app()


async def test_query_returns_nested_posts() -> None:
    """The generated `users` field resolves a post through the relationship."""
    engine = create_async_engine("sqlite+aiosqlite://")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        async with async_sessionmaker(engine, expire_on_commit=False)() as session:
            session.add(
                User(
                    name="Alice",
                    email="alice@example.com",
                    posts=[Post(title="Hello", content="First post", views=3)],
                )
            )
            await session.commit()
            result = await schema.execute("{ users { name posts { title views } } }", context_value=_Context(session))
    finally:
        await engine.dispose()

    assert result.errors is None
    assert result.data == {"users": [{"name": "Alice", "posts": [{"title": "Hello", "views": 3}]}]}
