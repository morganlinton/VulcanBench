# Rust control 2: formatter provenance

`control-2-formatted.rs` is exactly `control-1-compressed.rs` run through
rustfmt, with nothing edited by hand afterwards.

Formatter: `rustfmt 1.9.0-stable (59807616e1 2026-04-14)`, from the Rust 1.95.0
stable toolchain (`rustc 1.95.0 (59807616e 2026-04-14)`), edition 2021, default
configuration.

Command, run from the repository root:

```sh
D=docs/judging/controls-v3/rust
EMPTY="$(mktemp -d)" && : > "$EMPTY/rustfmt.toml"
rustfmt --edition 2021 --config-path "$EMPTY/rustfmt.toml" \
  < "$D/control-1-compressed.rs" > "$D/control-2-formatted.rs"
```

The empty `rustfmt.toml` passed with `--config-path` pins the default
configuration: it stops rustfmt from picking up any `rustfmt.toml` in the tree or
in the user's config directory. Input goes through stdin so the output carries
no file header.

`verify.sh` reruns this command and fails if the output differs from
`control-2-formatted.rs` by a single byte (it skips that check, and says so, when
the installed rustfmt is not 1.9.0). It also checks that `control-6-narrated.rs`
with its `//` comment lines removed is byte-identical to `control-2-formatted.rs`.

The other hand-written controls (0, 3, 4, 5, 7, 8, 9) are also rustfmt-clean
under the same command, so layout is not a variable between them; only control 1
is deliberately unformatted.
