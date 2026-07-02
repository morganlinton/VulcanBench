`pytest.approx(...)` has no public type, so users cannot write type annotations or `isinstance` checks against its return value.

`pytest.approx()` returns different internal objects depending on what you pass it: a scalar, a mapping, a sequence, a `Decimal`, or a numpy array each produce a different private class. There is currently no public, importable base type that unifies them, so downstream code cannot annotate a function that returns `approx(...)`, cannot `isinstance`-check a value against "is this an approx object", and cannot reference the type in documentation.

Expose `pytest.Approx` as a public type: the common base class of every object `pytest.approx()` can return. Requirements:

- `pytest.Approx` is importable from the top-level `pytest` package and listed in its `__all__`.
- For every input shape `approx()` supports (scalar, mapping, sequence, tuple, `Decimal`, numpy array when numpy is installed), `isinstance(pytest.approx(x), pytest.Approx)` is `True`.
- The existing approx comparison and rich-diff behavior is preserved and keys off this public type rather than the old private base.

Existing `approx()` comparisons and their failure diffs must continue to work unchanged.
