"""Baseline behavior that holds regardless of the precedence bug (pass_to_pass).

These cases either involve a single precedence level, are fully parenthesized,
or exercise tokenizing/error handling — so the buggy starting parser already
passes them. They must keep passing after the fix.
"""

import pytest

from calc import CalcError, evaluate


@pytest.mark.parametrize(
    "expr,want",
    [
        ("42", 42),
        ("3.5", 3.5),
        ("2 + 3 + 4", 9),
        ("10 - 3 - 2", 5),
        ("(2 + 3) * 4", 20),
        ("2 * (3 + 4)", 14),
        (" 1  +  2 ", 3),
        ("100 / 10 / 2", 5.0),
        ("2 ** -2", 0.25),  # unary minus inside an exponent
    ],
)
def test_basic_values(expr, want):
    got = evaluate(expr)
    assert got == want
    assert type(got) is type(want)


@pytest.mark.parametrize("expr", ["1 / 0", "5 % 0", "7 // 0"])
def test_division_by_zero_raises(expr):
    with pytest.raises(CalcError):
        evaluate(expr)


@pytest.mark.parametrize("expr", ["", "   ", "1 +", "(1", "1 2", "3 +* 2", "@", "1.2.3"])
def test_malformed_raises(expr):
    with pytest.raises(CalcError):
        evaluate(expr)
