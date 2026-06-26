"""RFC 3986 path normalization: many independent edge cases.

The bar is high on purpose: percent-encoding is normalized first (decode
unreserved octets, uppercase the hex of the rest, reject malformed), then
remove_dot_segments (RFC 3986 5.2.4) is applied, so percent-encoded dots such as
%2E participate in dot-segment removal.
"""

import pytest

from urlnorm import normalize_path


@pytest.mark.parametrize(
    "path,want",
    [
        ("/a/b/c/./../../g", "/a/g"),  # RFC 3986 5.2.4 example
        ("mid/content=5/../6", "mid/6"),  # RFC 3986 5.2.4 example
        ("/./", "/"),
        ("/../", "/"),
        ("/a/./b", "/a/b"),
        ("/a/../b", "/b"),
        ("/a/b/", "/a/b/"),
        ("/a/b/..", "/a/"),
        ("/a/b/c/../../../../g", "/g"),  # cannot go above root
        ("./g", "g"),
        ("../g", "g"),
        ("a/./b/../c", "a/c"),
        ("/foo/bar/../baz", "/foo/baz"),
        ("", ""),
        ("/", "/"),
    ],
)
def test_dot_segments(path, want):
    assert normalize_path(path) == want


@pytest.mark.parametrize(
    "path,want",
    [
        ("/a%62c", "/abc"),  # %62 -> 'b'
        ("/%41", "/A"),  # %41 -> 'A'
        ("/%7e", "/~"),  # tilde is unreserved
        ("/%2d%5f", "/-_"),  # '-' and '_' are unreserved
        ("/%30%39", "/09"),  # digits
    ],
)
def test_percent_unreserved_is_decoded(path, want):
    assert normalize_path(path) == want


@pytest.mark.parametrize(
    "path,want",
    [
        ("/%2f", "/%2F"),  # reserved slash: kept, hex uppercased
        ("/a%3db", "/a%3Db"),  # '=' kept, uppercased
        ("/%e2%82%ac", "/%E2%82%AC"),  # multi-byte stays encoded, uppercased
    ],
)
def test_percent_reserved_is_uppercased_not_decoded(path, want):
    assert normalize_path(path) == want


@pytest.mark.parametrize(
    "path,want",
    [
        ("/%2e%2e/x", "/x"),  # %2e%2e -> '..' -> removed
        ("/p/%2E/q", "/p/q"),  # %2E -> '.' -> removed
        ("/a/%2e%2e/%2e%2e/b", "/b"),
    ],
)
def test_percent_dot_interaction(path, want):
    assert normalize_path(path) == want


@pytest.mark.parametrize("path", ["/%2", "/%zz/", "/a%", "/%g0", "%"])
def test_malformed_percent_raises(path):
    with pytest.raises(ValueError):
        normalize_path(path)


def test_module_imports():
    assert callable(normalize_path)
