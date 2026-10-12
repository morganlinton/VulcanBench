# vyre-libs fixtures

Versioned KAT (Known Answer Test) fixtures for every Cat-A op's
conformance harness. Each fixture TOML file:

1. Declares `schema_version = "1"` at the top. Bumps are semver-
   breaking changes to the fixture layout.
2. Cites the upstream source of each witness vector so reviewers
   can audit provenance.
3. Stores test inputs + expected outputs as structured data rather
   than hardcoded arrays in test source files.

Format:

```toml
schema_version = "1"
op = "vyre-libs::matching::aho_corasick"
source = "Aho-Corasick 1975 paper + aho-corasick crate regression corpus"

[[witness]]
name = "ac-paper-ushers"
patterns = ["he", "she", "his", "hers"]
haystack = "ushers"
# expected_accepts is the accept[state] value at each haystack byte
expected_accepts = [0, 0, 0, 3, 1, 4]
```

External op authors ship fixtures in their own crate's `fixtures/`
directory following the same schema.

## Current fixture files

- `aho_corasick.toml`: 20 regression vectors from
  `tests/aho_corasick_kat.rs`.
- `blake3.toml`: 3 KAT vectors from `tests/blake3_kat.rs`.

## Why TOML

- Diff-reviewable: any reviewer can audit a PR that touches a
  fixture byte-by-byte.
- Loadable from non-Rust tooling for parity runs outside the vyre
  test harness.
- `#[schema_version]` gates future migrations cleanly.
