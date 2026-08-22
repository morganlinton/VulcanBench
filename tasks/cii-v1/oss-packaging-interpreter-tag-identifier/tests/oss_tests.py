"""Hidden fail-to-pass tests: non-identifier interpreter components must be
rejected. Assertions check exception types via public APIs, never messages."""

import pytest

from packaging import tags, utils


@pytest.mark.parametrize(
    "tag",
    ["2-none-any", "2.7.6-none-any", "py3.2-none-any", "py+3-none-any"],
)
def test_non_identifier_interpreter_rejected(tag):
    with pytest.raises(tags.InvalidTag):
        tags.parse_tag(tag)


def test_dotted_interpreter_not_split_into_tags():
    # At minimum, '2.7.6' must never be treated as a compressed set of the
    # interpreters 2, 7 and 6.
    try:
        result = tags.parse_tag("2.7.6-none-any")
    except tags.InvalidTag:
        return
    assert tags.Tag("7", "none", "any") not in result


def test_wheel_filename_invalid_interpreter_rejected():
    with pytest.raises(utils.InvalidWheelFilename):
        utils.parse_wheel_filename("playlyfe-0.1.1-2.7.6-none-any.whl")


def test_wheel_filename_plus_interpreter_rejected():
    with pytest.raises(utils.InvalidWheelFilename):
        utils.parse_wheel_filename("foo-1.0-py+3-none-any.whl")
