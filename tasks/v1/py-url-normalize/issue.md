# Implement RFC 3986 URL path normalization

`urlnorm.normalize_path(path)` should return the normalized form of a URL path
string. Apply these two steps, **in this order**:

1. **Percent-encoding normalization.** Scan the string for `%HH` escapes.
   - If `HH` decodes to an *unreserved* character (`A-Z a-z 0-9 - . _ ~`),
     replace the escape with that literal character.
   - Otherwise keep the escape but upper-case its two hex digits (`%2f` ->
     `%2F`). Bytes that are part of a multi-byte sequence stay encoded.
   - A `%` not followed by two hex digits is malformed: raise `ValueError`.

2. **Dot-segment removal** (RFC 3986 section 5.2.4). Resolve `.` and `..`
   segments. `..` may not rise above the root of an absolute path. Examples:
   `"/a/b/c/./../../g"` -> `"/a/g"`, `"mid/content=5/../6"` -> `"mid/6"`,
   `"/a/b/.."` -> `"/a/"`, `"/a/b/c/../../../../g"` -> `"/g"`.

Because percent-normalization happens first, an encoded dot participates in
dot-segment removal: `"/%2e%2e/x"` normalizes to `"/x"` (the `%2e%2e` decodes to
`..`). The empty string normalizes to the empty string.

Implement `normalize_path` in `urlnorm/normalize.py`.
