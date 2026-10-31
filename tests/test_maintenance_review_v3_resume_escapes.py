"""recover_escaped_excerpts: a quoted string literal whose escapes the judge's JSON decoded.

The two real cases: v3.18 Muse probe submission-083 (codeccore) quoted a line
whose source spells NUL as b"\\0"; v3.29 Muse probe submission-011 (settlecore)
quoted one whose source spells it b"\\x00". Both decode to the same control
characters, so each spelling must be tried.
"""

from __future__ import annotations

from harness.maintenance_review_v3_resume import _escape_variants, rewrap_excerpt

DECODED = 'raw = chunk.split(b"\x00", 1)[0].split(b"\r", 1)[0].split(b"\n", 1)[0]'
CODECCORE = 'raw = chunk.split(b"\\0", 1)[0].split(b"\\r", 1)[0].split(b"\\n", 1)[0]'
SETTLECORE = 'raw = chunk.split(b"\\x00", 1)[0].split(b"\\r", 1)[0].split(b"\\n", 1)[0]'


def recovered(excerpt: str, source: list[str]) -> str | None:
    return next(
        (hit for v in _escape_variants(excerpt) if (hit := rewrap_excerpt(v, source)) is not None),
        None,
    )


def source_with(line: str) -> list[str]:
    return [f"def first_field(chunk):\n    {line}\n    return raw\n"]


def test_backslash_zero_spelling_comes_first() -> None:
    variants = _escape_variants(DECODED)
    assert CODECCORE in variants and SETTLECORE in variants
    assert variants.index(CODECCORE) < variants.index(SETTLECORE)


def test_each_source_spelling_is_recovered() -> None:
    # The first variant keeps a decoded line break as a line break, which the frozen
    # check reads as two verbatim fragments; either way the NUL spelling must match.
    codeccore = recovered(DECODED, source_with(CODECCORE))
    settlecore = recovered(DECODED, source_with(SETTLECORE))
    assert codeccore is not None and 'b"\\0"' in codeccore
    assert settlecore is not None and 'b"\\x00"' in settlecore


def test_backslash_zero_alone_misses_the_hex_spelling() -> None:
    zero_only = [v for v in _escape_variants(DECODED) if "\\x00" not in v]
    assert all(rewrap_excerpt(v, source_with(SETTLECORE)) is None for v in zero_only)


def test_leading_indentation_is_still_verbatim() -> None:
    # Attempt 2 on settlecore quoted the line with its indentation.
    assert recovered("    " + DECODED, source_with(SETTLECORE)) is not None


def test_changed_token_is_not_recovered() -> None:
    assert recovered(DECODED.replace("chunk", "block"), source_with(SETTLECORE)) is None


def test_excerpt_without_control_characters_has_no_variant() -> None:
    assert _escape_variants(SETTLECORE) == []
