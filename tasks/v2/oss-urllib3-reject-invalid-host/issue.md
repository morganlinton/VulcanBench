`parse_url` accepts hosts that contain characters that can never be valid in a hostname, which lets malformed and potentially dangerous URLs through.

Today, parsing a URL whose host contains a raw space, control character, tab, or DEL byte succeeds and returns the bad bytes verbatim in `.host`. The same is true for a host with a percent-encoded control character (e.g. `%00`). Downstream code then builds requests against a host that no server can serve and that can enable request-splitting style problems.

Make host parsing reject invalid hosts by raising `LocationParseError`:

- A host containing any raw character in the C0 control range or space (`\x00` through `\x20`) or DEL (`\x7f`) must raise.
- A host containing a percent-encoded control character (a `%XX` escape that decodes to a `\x00`-`\x1f` or `\x7f` byte) must raise.
- A host containing a stray `%` that is not a valid `%XX` escape must raise.

Legitimate hosts must continue to parse unchanged, including hosts that use valid percent-encoding such as `%20`. Apply the same validation whether the host is well-formed enough to match the host/port grammar or not.
