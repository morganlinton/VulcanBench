# Port GitHub Flavored Markdown tables and autolinks to match the C reference exactly

The workspace at `/app` is comrak, a Rust implementation of CommonMark and
GitHub Flavored Markdown (GFM). Its support for two GFM extensions has been
removed and must be rebuilt:

- **tables** (`-e table`): pipe tables with a delimiter row, column
  alignment, escaped pipes, rows with too few or too many cells, and how a
  table starts, continues and ends next to other blocks;
- **extended autolinks** (`-e autolink`): `www.` links, `http://`,
  `https://` and `ftp://` links, and email addresses recognised in plain
  text, including where a link ends next to punctuation, brackets and
  entities.

Right now `src/parser/table.rs` and `src/parser/autolink.rs` are stubs, so
no table and no extended autolink is ever recognised. Everything else in
comrak works and must keep working.

The behaviour to reproduce is that of the reference implementation,
`cmark-gfm` 0.29.0.gfm.13 (GitHub's C implementation), whose complete source
is in `/app/reference/cmark-gfm` (the extensions live under `extensions/`,
and they interact with the core parser in `src/`). The target's HTML must be
**byte-for-byte identical** to the reference's: same elements, attributes,
escaping, whitespace and newlines. Where the reference does something
surprising, match the reference. The CommonMark specification is not the
standard here; the reference's code is.

The build works offline (`cargo build --release --bin comrak`). A visible
sample of 150 cases with the reference's output is in
`/app/corpus/visible_corpus.jsonl`; `python3 /app/corpus/check_visible.py`
runs the target on it and shows mismatches. Grading uses a much larger
hidden corpus that exercises the same features in many more combinations,
so matching the visible sample is not enough: port the behaviour, do not
special-case the examples.

Grading builds only your `src` tree, with the original `Cargo.toml`,
`Cargo.lock` and `build.rs`, and runs

```
comrak --unsafe --syntax-highlighting none --gfm-quirks [-e EXT ...] [--smart] < input.md
```

for every case, comparing standard output with the reference's. A case
passes only on an exact match. Every case must pass, including the many
that do not involve tables or autolinks and already match today. Do not add
dependencies, read files or start processes at run time, or call into C:
the target must implement the behaviour itself in Rust.
