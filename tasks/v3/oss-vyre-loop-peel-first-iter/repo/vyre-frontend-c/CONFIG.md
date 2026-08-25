# vyre-frontend-c  -  Configurability

The `vyrec` binary (workspace `tools/vyrec`) and the `vyre-frontend-c` library
expose Tier A operational knobs and consume the workspace Tier-B op
corpus.

## Tier A  -  operational config

`vyrec` follows the standard cc/clang-style flags wherever the meaning
is identical, and adds vyre-specific extensions.

| Flag                            | Default      | Purpose                                                                    |
|---------------------------------|--------------|----------------------------------------------------------------------------|
| `-c`                            | off          | Compile-only: emit a `.o` artifact, skip link.                            |
| `-o <path>`                     | `a.out` / `<src>.o` | Output path.                                                               |
| `-I <dir>`                      | repeat       | Add include dir to the search path.                                        |
| `-include <file>`               | repeat       | Force-include the file before TU body.                                     |
| `-D NAME[=VALUE]`               | repeat       | Define a macro.                                                            |
| `-U NAME`                       | repeat       | Undefine a macro evaluated after `-D`.                                     |
| env `VYRE_CC_TIMEOUT_SEC`       | `120`        | Per-TU GPU compilation deadline.                                           |
| env `VYRE_CC_PARALLELISM`       | `nproc`      | Host-side parallelism for TU prep + linking.                               |
| env `VYRE_CC_DUMP_INTERMEDIATES`| `0`          | `1` = preserve `VYRECOB1`/`VYRECOB2` payloads alongside outputs.          |

Compiled defaults < `vyre-frontend-c.toml` < env < CLI. The on-disk default
config lives at `tools/vyrec/.vyre-frontend-c.toml` and is documented inline.

## Tier B  -  community knowledge

`vyre-frontend-c` consumes the workspace op corpus (`rules/op/*.toml`) for the
GPU C spine: lex, digraph rewrite, conditional mask, macro expansion,
bracket match, function-shape, ABI layout, AST shunting yard, CFG
construction, ELF lowering. Each stage references an op via TOML; new
op coverage extends the spine without touching Rust.

The grammar tables themselves come from an external grammar-table generator, which
emits TOML grammar files into `rules/grammar/`. Grammar additions are
TOML-only.
