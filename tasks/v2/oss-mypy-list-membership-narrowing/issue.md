mypy narrows a value inside `if x in (...)` when the right-hand side is a tuple literal, but not when it is a list literal, even though the two are equivalent for a membership test.

For example:

```python
from typing import Literal

def f(x: Literal['a', 'b', 'c', 'd']) -> None:
    if x in ('a', 'b'):
        reveal_type(x)   # Literal['a', 'b']  (already works)
    if x in ['a', 'b']:
        reveal_type(x)   # Literal['a', 'b', 'c', 'd']  (should be Literal['a', 'b'])
```

Inside the positive branch of a membership test against a **list** literal, mypy should narrow the tested value to the union of the literal items it could match, exactly as it does for a tuple literal. This applies to any literal element type (string literals, int literals, etc.).

The existing tuple-literal narrowing behavior must be preserved unchanged.
