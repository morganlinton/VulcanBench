class CalcError(Exception):
    """Raised on malformed input or a runtime evaluation error.

    Examples: an unexpected or trailing token, unbalanced parentheses, an empty
    expression, or division/modulo by zero.
    """
