"""Hidden pass-to-pass guards: valid tags and existing rejections unchanged."""

import pytest

from packaging import tags, utils


@pytest.mark.parametrize("interpreter", ["sillywalk", "graalpy311", "_custom", "py3", "cp312"])
def test_identifier_interpreters_accepted(interpreter):
    assert tags.parse_tag(f"{interpreter}-none-any") == {tags.Tag(interpreter, "none", "any")}


def test_compressed_tag_sets_still_expand():
    result = tags.parse_tag("py3.cp312-none-any")
    assert result == {tags.Tag("py3", "none", "any"), tags.Tag("cp312", "none", "any")}


def test_empty_component_still_rejected():
    with pytest.raises(tags.InvalidTag):
        tags.parse_tag("-none-any")


def test_wrong_component_count_still_rejected():
    with pytest.raises(tags.InvalidTag):
        tags.parse_tag("py3-none")


def test_valid_wheel_filename_still_parses():
    name, version, build, wheel_tags = utils.parse_wheel_filename("foo-1.0-py3-none-any.whl")
    assert name == "foo"
    assert str(version) == "1.0"
    assert wheel_tags == frozenset({tags.Tag("py3", "none", "any")})
