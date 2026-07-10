Add `strip_prefix` and `strip_prefix_by` to the `Itertools` trait.

`str` has `strip_prefix`, but there is no iterator equivalent. Add the iterator
analogue to the `Itertools` trait (in `src/lib.rs`), along with a public error
type for the mismatch case (see #1096).

Add two provided methods:

- `fn strip_prefix<Prefix>(self, prefix: Prefix) -> Result<Self, StripPrefixError<...>>`
  where `Prefix: IntoIterator` and `Self::Item: PartialEq<Prefix::Item>`.
- `fn strip_prefix_by<Prefix, F>(self, prefix: Prefix, eq: F) -> Result<Self, StripPrefixError<...>>`
  where `F: FnMut(&Self::Item, &Prefix::Item) -> bool`. `strip_prefix` is just
  `strip_prefix_by` with `|a, b| a == b`. The predicate form lets the prefix
  items be a different type than `Self::Item`.

Behavior:

- If the iterator begins with every item yielded by `prefix` (in order, as
  judged by equality/`eq`), return `Ok(self)` with the iterator advanced past
  the matched prefix. An empty prefix strips nothing; a full-length prefix
  leaves an empty iterator.
- On the first mismatch, return `Err(StripPrefixError { iterator, prefix,
  mismatch })`:
  - `iterator`: the remainder of the original iterator, advanced past the items
    that matched and stopped at the mismatch position;
  - `prefix`: the remainder of the prefix iterator, after the prefix item that
    failed to match;
  - `mismatch`: the `(Option<Self::Item>, Prefix::Item)` pair that failed to
    compare equal. The first element is `None` if the iterator was exhausted
    before the prefix was fully consumed.

Add a public `StripPrefixError<I, Prefix: Iterator, T>` struct with public
`iterator`, `prefix`, and `mismatch` fields (derive `Debug` and `Clone`), so
callers can recover the partially-consumed state.

Examples:

```rust
use itertools::Itertools;

let rest = (1..6).strip_prefix([1, 2]).unwrap().collect_vec();
assert_eq!(rest, vec![3, 4, 5]);

assert!((1..6).strip_prefix([1, 9]).is_err());

let path = ["home", "user", "file"];
let stripped = path.iter().strip_prefix_by(["home", "user"], |a, b| **a == *b);
assert_eq!(stripped.map(Itertools::collect_vec).ok(), Some(vec![&"file"]));
```

Existing `Itertools` methods must be unaffected.
