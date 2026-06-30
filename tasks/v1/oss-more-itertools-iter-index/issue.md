# `iter_index` raises on negative start/stop for general iterables

`more_itertools.iter_index(iterable, value, start=0, stop=None)` yields the
indices at which `value` appears in `iterable`. When `iterable` is a sequence
(it has an `.index` method) negative `start`/`stop` work with the usual
from-the-end semantics. But for a general iterable (for example an iterator),
negative bounds raise instead:

```python
>>> list(iter_index(list('AABCADEAF'), 'A', start=-3))
[7]
>>> list(iter_index(iter('AABCADEAF'), 'A', start=-3))
ValueError: Indices for islice() must be None or an integer: 0 <= x <= sys.maxsize.
```

The two code paths should agree. For a general iterable, negative `start`/`stop`
must produce the same result as the equivalent sequence, matching built-in
`list.index` / `str.index` from-the-end semantics (so the iterator example above
should also yield `[7]`). Positive bounds and the default behavior must keep
working unchanged.

`iter_index` lives in `more_itertools/recipes.py`.
