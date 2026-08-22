# Add a `regex!` macro for automatic reuse of a compiled regex

Compiling a regex is expensive, and the documented way to reuse one across
calls is a `std::sync::LazyLock` static at every call site. Add the macro
users keep re-implementing (upstream issue #709):

```rust
use regex::regex;

fn is_match(line: &str) -> bool {
    regex!(r"bar|baz").is_match(line)
}
```

- `regex!(r"...")` takes a pattern literal and yields a plain `&Regex`,
  lazily compiled on first use and cached in a static owned by that call
  site — repeated calls through the same site reuse one compilation, and
  several `regex!` invocations can coexist in one scope.
- An invalid pattern panics on first use (the macro takes a literal, so
  this is a programming error, not input handling).
- A `bytes::regex!` twin yields `&bytes::Regex` for `&[u8]` haystacks.
- The returned reference is an ordinary `&Regex`: captures, named groups
  and every other method work as usual, and it can be passed to functions
  expecting `&Regex`. Don't leak internal supporting types into the public
  API surface.
- `Regex::new` and friends are unchanged.
