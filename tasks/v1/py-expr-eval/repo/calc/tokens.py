"""Tokenizer for the arithmetic mini-language.

Tokens: NUMBER (int or float), the operators ``+ - * / // % **``, and
parentheses. Whitespace separates tokens but is otherwise insignificant. This
module is correct — bugs in this task live in the parser, not the lexer.
"""

from __future__ import annotations

from dataclasses import dataclass

from calc.errors import CalcError

# Two-character operators must be matched before their one-character prefixes.
_TWO_CHAR = {"**": "DSTAR", "//": "DSLASH"}
_ONE_CHAR = {
    "+": "PLUS",
    "-": "MINUS",
    "*": "STAR",
    "/": "SLASH",
    "%": "PERCENT",
    "(": "LPAREN",
    ")": "RPAREN",
}


@dataclass(frozen=True)
class Token:
    kind: str
    value: str | float | int


def tokenize(text: str) -> list[Token]:
    tokens: list[Token] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
            continue
        pair = text[i : i + 2]
        if pair in _TWO_CHAR:
            tokens.append(Token(_TWO_CHAR[pair], pair))
            i += 2
            continue
        if c in _ONE_CHAR:
            tokens.append(Token(_ONE_CHAR[c], c))
            i += 1
            continue
        if c.isdigit() or c == ".":
            j = i
            seen_dot = False
            while j < n and (text[j].isdigit() or text[j] == "."):
                if text[j] == ".":
                    if seen_dot:
                        raise CalcError(f"malformed number near {text[i:j+1]!r}")
                    seen_dot = True
                j += 1
            literal = text[i:j]
            if literal == ".":
                raise CalcError("malformed number '.'")
            value: float | int = float(literal) if seen_dot else int(literal)
            tokens.append(Token("NUMBER", value))
            i = j
            continue
        raise CalcError(f"unexpected character {c!r} at position {i}")
    tokens.append(Token("EOF", ""))
    return tokens
