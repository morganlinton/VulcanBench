"""Semantic-version precedence (semver.org section 11), the subtle parts.

The hard parts are the prerelease rules: a prerelease is lower than its release,
numeric identifiers are lower than alphanumeric ones, numeric identifiers compare
numerically (so 2 < 11), and when all shared identifiers are equal the version
with more identifiers wins. Build metadata is ignored.
"""

import pytest

from semver import compare

# Strictly increasing precedence chain from semver.org.
CHAIN = [
    "1.0.0-alpha",
    "1.0.0-alpha.1",
    "1.0.0-alpha.beta",
    "1.0.0-beta",
    "1.0.0-beta.2",
    "1.0.0-beta.11",
    "1.0.0-rc.1",
    "1.0.0",
]


@pytest.mark.parametrize(
    "a,b,want",
    [
        ("2.0.0", "1.9.9", 1),
        ("1.2.0", "1.10.0", -1),
        ("1.0.1", "1.0.0", 1),
        ("1.0.0", "1.0.0", 0),
    ],
)
def test_core_precedence(a, b, want):
    assert compare(a, b) == want


def test_prerelease_is_lower_than_release():
    assert compare("1.0.0-alpha", "1.0.0") == -1
    assert compare("1.0.0", "1.0.0-rc.1") == 1


@pytest.mark.parametrize("i", range(len(CHAIN) - 1))
def test_prerelease_precedence_chain(i):
    assert compare(CHAIN[i], CHAIN[i + 1]) == -1
    assert compare(CHAIN[i + 1], CHAIN[i]) == 1  # symmetry


@pytest.mark.parametrize(
    "a,b,want",
    [
        ("1.0.0-alpha.1", "1.0.0-alpha.beta", -1),  # numeric < alphanumeric
        ("1.0.0-beta.2", "1.0.0-beta.11", -1),  # numeric compared numerically
        ("1.0.0-alpha", "1.0.0-alpha.1", -1),  # more fields wins
    ],
)
def test_prerelease_identifier_rules(a, b, want):
    assert compare(a, b) == want


@pytest.mark.parametrize(
    "a,b",
    [
        ("1.0.0+build.1", "1.0.0+build.2"),
        ("1.0.0", "1.0.0+meta"),
        ("1.0.0-alpha+001", "1.0.0-alpha+999"),
    ],
)
def test_build_metadata_ignored(a, b):
    assert compare(a, b) == 0


@pytest.mark.parametrize(
    "bad",
    ["1.0", "1.2.3.4", "01.0.0", "1.0.0-", "1.0.0-01", "1.0.0-a..b", "", "v1.0.0", "1.0.0 "],
)
def test_invalid_raises(bad):
    with pytest.raises(ValueError):
        compare(bad, "1.0.0")


def test_module_imports():
    assert callable(compare)
