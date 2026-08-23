# url() returns a different string than it validated; ipv6() validates something else entirely

Two related holes in address validation, both rooted in leaning on the
WHATWG URL parser:

1. **The parser deletes ASCII tab, LF and CR instead of failing.**
   `new URL("https://exa\nmple.com")` reports on `example.com`, but
   `z.url()` returns the ORIGINAL string — a value that names a different
   host than the one that was validated:

   ```ts
   z.url().parse("https://exa\nmple.com")
   // returns "https://exa\nmple.com" — validated host was example.com
   ```

2. **`new URL("http://[...]")` parses an authority, not an address.**
   Delimiters re-split it, so `z.ipv6()` accepts garbage:

   ```ts
   z.ipv6().safeParse("::@1\\").success // true — validated the host 0.0.0.1
   z.ipv6().safeParse("::1\n").success  // true — parser deleted the newline
   ```

Expected:

- `z.url()` (and `httpUrl`) returns the tab/LF/CR-stripped string the
  parser actually validated — in the interpreted and compiled paths alike.
  Clean URLs are returned verbatim; `normalize: true` still wins where it
  applies; nothing that parsed before stops parsing.
- `z.ipv6()` / `z.cidrv6()` reject any input containing characters outside
  the address alphabet (hex digits, colons, dots — plus the prefix for
  CIDR), including tab/LF/CR and re-delimiting characters. Valid addresses
  are unchanged.
