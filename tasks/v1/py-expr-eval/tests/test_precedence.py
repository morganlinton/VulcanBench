"""Operator precedence and associativity (fail_to_pass).

The buggy starting parser folds every operator at one precedence level, left to
right, so each of these evaluates incorrectly until the grammar is fixed.
"""

from calc import evaluate


def test_add_then_mul():
    # multiplication binds tighter than addition: 2 + (3*4)
    assert evaluate("2 + 3 * 4") == 14


def test_sub_then_mul():
    assert evaluate("10 - 2 * 3") == 4


def test_mul_then_pow():
    # exponent binds tighter than multiplication: 2 * (3**2)
    assert evaluate("2 * 3 ** 2") == 18


def test_pow_is_right_associative():
    # 2 ** (3 ** 2) == 2 ** 9 == 512, not (2**3)**2 == 64
    assert evaluate("2 ** 3 ** 2") == 512


def test_unary_minus_below_power():
    # -2 ** 2 == -(2 ** 2) == -4, not (-2) ** 2 == 4
    assert evaluate("-2 ** 2") == -4


def test_add_then_pow():
    assert evaluate("2 + 2 ** 3") == 10


def test_mixed_chain():
    assert evaluate("1 + 2 * 3 - 4") == 3


def test_true_division_precedence():
    got = evaluate("1 + 6 / 2")
    assert got == 4.0
    assert isinstance(got, float)


def test_floor_division_precedence():
    assert evaluate("10 - 7 // 2") == 7


def test_modulo_precedence():
    assert evaluate("10 + 7 % 3") == 11
