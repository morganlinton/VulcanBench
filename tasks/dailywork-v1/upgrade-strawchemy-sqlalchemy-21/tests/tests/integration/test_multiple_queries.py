from __future__ import annotations

import pytest

from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

pytestmark = [pytest.mark.integration]


async def test_create_update_delete(any_query: AnyQueryExecutor) -> None:
    color_id = 99999

    create_query = """
        mutation {{
            createColor(data: {{ id: {id}, name: "New Blue" }}) {{
                name
            }}
        }}
    """

    update_query = """
        mutation {{
            updateColor(
                data: {{
                    id: {id},
                    name: "New Green"
                }}
            ) {{
                id
                name
            }}
        }}
    """

    get_query = """
        query {{
            color(id: {id}) {{
                name
            }}
        }}
    """

    delete_query = """
        mutation {{
            deleteColor(
                filter: {{
                    id: {{ eq: {id} }}
                }}
            ) {{
                id
                name
            }}
        }}
    """

    list_query = """
        query {{
            colors(filter: {{ id: {{ eq: {id} }} }}) {{
                id
                name
            }}
        }}
    """

    # Create
    result = await maybe_async(any_query(create_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["createColor"] == {"name": "New Blue"}
    # Get
    result = await maybe_async(any_query(get_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["color"] == {"name": "New Blue"}
    # Update
    result = await maybe_async(any_query(update_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["updateColor"] == {"name": "New Green", "id": color_id}
    # Get
    result = await maybe_async(any_query(get_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["color"] == {"name": "New Green"}
    # Delete
    result = await maybe_async(any_query(delete_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["deleteColor"] == [{"name": "New Green", "id": color_id}]
    # List
    result = await maybe_async(any_query(list_query.format(id=color_id)))
    assert not result.errors
    assert result.data
    assert result.data["colors"] == []
