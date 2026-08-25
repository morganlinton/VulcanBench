# TICKET R2  -  Linux kernel `scripts/` corpus inventory

## Summary

| Metric          | Count |
|-----------------|-------|
| Total files     | 47    |
| GCC pass        | 6     |
| vyrec pass      | 1     |
| vyrec fail      | 43    |
| Skipped (>50KB) | 3     |

*Corpus source:* `/usr/src/linux-headers-6.17.0-14-generic/scripts/`
*vyrec invocation:* `vyrec -c -o /tmp/out.o -I /usr/include -I /usr/include/x86_64-linux-gnu <file>`
*gcc invocation:* `gcc -c -o /tmp/out.o -I /usr/include -I /usr/include/x86_64-linux-gnu <file>`

## SKIPPED (too large)

| File | Size |
|------|------|
| `kconfig/lexer.lex.c` | 113,120 bytes |
| `kconfig/parser.tab.c` | 73,566 bytes |
| `mod/modpost.c` | 60,123 bytes |

## vyrec failure clusters

| Count | Normalized message | Example file |
|-------|-------------------|--------------|
| 25 | `vyrec fatal error: vyre-frontend-c: system #include <gnu/stubs-32.h> not found in -I search path` | `basic/fixdep.c` |
| 5 | `vyrec fatal error: vyre-frontend-c: #include "dialog.h" not found (tried TU dir and -I)` | `kconfig/lxdialog/checklist.c` |
| 4 | `vyrec fatal error: vyre-frontend-c: system #include <stdarg.h> not found in -I search path` | `asn1_compiler.c` |
| 2 | `vyrec fatal error: vyre-frontend-c: #include "gendwarfksyms.h" not found (tried TU dir and -I)` | `gendwarfksyms/cache.c` |
| 2 | `vyrec fatal error: vyre-frontend-c: system #include <xalloc.h> not found in -I search path` | `kconfig/nconf.gui.c` |
| 1 | `vyrec fatal error: vyre-frontend-c: system #include <stdbool.h> not found in -I search path` | `gen_packed_field_checks.c` |
| 1 | `vyrec fatal error: vyre-frontend-c: #include "images.h" not found (tried TU dir and -I)` | `kconfig/images.c` |
| 1 | `vyrec fatal error: vyre-frontend-c: system #include <list.h> not found in -I search path` | `kconfig/mnconf-common.c` |
| 1 | `vyrec fatal error: vyre-frontend-c: system #include <linux/kbuild.h> not found in -I search path` | `mod/devicetable-offsets.c` |
| 1 | `vyrec fatal error: vyre-frontend-c: system #include <linux/build-salt.h> not found in -I search path` | `module-common.c` |

## Observations

- **System include defaults missing (P1 gap):** The dominant failure mode (29/43 failures) is missing system headers (`<gnu/stubs-32.h>`, `<stdarg.h>`, `<stdbool.h>`, `<xalloc.h>`, `<list.h>`, `<linux/kbuild.h>`, `<linux/build-salt.h>`). These headers are available on the host but vyrec does not search the gcc default system-include paths unless explicitly passed via `-I`. This is the exact gap described in ticket **P1** (system-include default paths).
- **Local sibling headers missing:** 8 failures are for quoted `#include "*.h"` of headers that live in the same source tree (`dialog.h`, `gendwarfksyms.h`, `images.h`). These are local project headers that gcc also fails to find when compiled in isolation. Adding the correct `-I` paths for each subdirectory would resolve both gcc and vyrec for these files.
- **GCC also fails on most files:** Only 6 of 44 files compile with gcc in isolation because the kernel scripts tree relies on sibling headers and generated headers not present in a single-directory view.
- **Single vyrec success:** `mod/empty.c` (54 bytes, essentially empty) is the only file that passes vyrec. It also passes gcc.

## OUT_OF_SCOPE_FINDING

- `vyre-frontend-c/Cargo.toml` was missing the `vyre-foundation` dependency required by `src/pipeline.rs` (used by commit `c7313ac92f`). Added `vyre-foundation = { path = "../vyre-foundation" }` to make `cargo build -p vyrec --release` succeed. No parser or compiler logic was modified.
