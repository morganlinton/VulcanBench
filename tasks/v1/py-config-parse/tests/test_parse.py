"""VulcanConf parser: many independent, subtly-interacting rules.

Each test group covers a distinct rule. The format is invented for this task, so
it cannot be matched to a known config spec; correctness comes from applying
every rule exactly, including the interactions (quoted vs unquoted coercion, the
whitespace-before-# inline-comment rule, leading-zero and bare-dot number rules).
"""

import pytest

from confkit import parse_config


def test_comments_and_blanks():
    assert parse_config("# top\n\n   # indented\na = 1\n") == {"a": 1}


@pytest.mark.parametrize(
    "text,want",
    [
        ("a=hello", {"a": "hello"}),
        ("  a   =   hello world  ", {"a": "hello world"}),  # trailing trimmed, inner kept
        ("a = x  y", {"a": "x  y"}),
    ],
)
def test_basic_and_whitespace(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize(
    "text,want",
    [
        ('a = "  hi  "', {"a": "  hi  "}),  # quotes preserve whitespace
        ('a = "x#y"', {"a": "x#y"}),  # '#' inside quotes is not a comment
        ('a = "42"', {"a": "42"}),  # quoted value is always a string
        ('a = "true"', {"a": "true"}),
    ],
)
def test_quoting(text, want):
    assert parse_config(text) == want


def test_escapes_in_quotes():
    assert parse_config('a = "p\\nq\\tr\\"s\\\\t"') == {"a": 'p\nq\tr"s\\t'}
    assert parse_config('a = "z\\qy"') == {"a": "zqy"}  # unknown escape drops the backslash


@pytest.mark.parametrize(
    "text,want",
    [
        ("a = val # trailing", {"a": "val"}),  # '#' after whitespace starts a comment
        ("a = x#y", {"a": "x#y"}),  # '#' with no leading whitespace is literal
        ('a = "ab" # c', {"a": "ab"}),  # comment allowed after a quoted value
    ],
)
def test_inline_comments(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize(
    "text,want",
    [
        ("a = true\nb = FALSE", {"a": True, "b": False}),
        ("a = null\nb = None", {"a": None, "b": None}),
    ],
)
def test_bool_null_coercion(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize(
    "text,want",
    [
        ("a = 42\nb = -7\nc = +3\nd = 0", {"a": 42, "b": -7, "c": 3, "d": 0}),
        ("a = 007", {"a": "007"}),  # leading zeros are not an int
        ("a = 00", {"a": "00"}),
    ],
)
def test_int_coercion(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize(
    "text,want",
    [
        ("a = 1.5\nb = 1e3\nc = 2.5e-2", {"a": 1.5, "b": 1000.0, "c": 0.025}),
        ("a = 5.\nb = .5", {"a": "5.", "b": ".5"}),  # need digits both sides of '.'
    ],
)
def test_float_coercion(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize(
    "text,want",
    [
        ("a =", {"a": ""}),
        ("a = # only a comment", {"a": ""}),
        ("a = 1\na = 2", {"a": 2}),  # duplicate key, last wins
    ],
)
def test_empty_and_duplicate(text, want):
    assert parse_config(text) == want


@pytest.mark.parametrize("text", ["novalue", "= v", 'a = "abc', 'a = "ab" cd'])
def test_invalid_raises(text):
    with pytest.raises(ValueError):
        parse_config(text)


def test_module_imports():
    assert callable(parse_config)
