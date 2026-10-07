# Sign inference is wrong for sums of several same-signed terms

The workspace at `/app` is SymPy, a pure-Python symbolic mathematics library.
Install the test dependencies and run the relevant suites:

```
python -m pip install mpmath pytest hypothesis
python -m pytest sympy/core/tests/test_exprtools.py
```

They pass as given. But SymPy draws the wrong conclusions about the sign of a
sum when several terms share a sign and a constant is added. For positive
integer symbols `a`, `b`, `c`:

```python
>>> from sympy import symbols
>>> a, b, c = symbols('a b c', positive=True, integer=True)
>>> (a + b + c - 2).is_positive      # a,b,c >= 1 so the sum is >= 3 > 2
None                                   # should be True
>>> (a + b + c - 3).is_nonnegative    # sum >= 3
None                                   # should be True
>>> (-a - b - c + 3).is_nonpositive
None                                   # should be True
```

The underlying monotonic-sign reasoning does not combine the guaranteed bounds
of several same-signed terms, so it fails to prove signs it should, while it
must still return `None` (unknown) for genuinely indeterminate cases such as
`(a + b - c - 2).is_positive`. Make the sign inference derive the correct
`is_positive` / `is_nonnegative` / `is_negative` / `is_nonpositive` results for
sums of several same-signed bounded terms, without ever claiming a sign that is
not actually guaranteed, and without changing results for existing cases.

Grading rebuilds the library from your workspace in a clean environment, runs a
held-out set of sign-inference tests, and runs the existing expression-tools
test suite as a regression wall. Only changes under `sympy/` are graded.
