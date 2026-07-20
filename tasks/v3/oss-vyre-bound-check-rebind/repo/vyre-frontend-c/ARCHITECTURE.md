# vyre-frontend-c  -  architecture

Compile vyre `Program`s into native object files so the program
can be linked against any C/C++/Rust toolchain. Distinct from
`vyre-aot`: that ships GPU artifacts; `vyre-frontend-c` ships CPU
host-side object files for in-process use.

## Modules

### `api/`
Public Rust surface: a small builder API around the compile-and-
link sequence. Returns a static-library archive or a dynamic
library, indexable by op id.

### `pipeline/` + `pipeline.rs`
Compile pipeline stages. Lowers `Program` → C source → invokes
the host C compiler (clang preferred, gcc fallback) → emits a
PIC object → archives.

### `tu_host/` + `tu_host.rs`
Translation-unit host interface  -  generates the per-op C
prototypes and binds them through the host's symbol table so a
linked program can call them by name.

### `elf_linux.rs`
Linux-specific ELF emission. Wraps the platform linker and
adds a `.note.vyre.cc` section carrying the conformance cert.

### `object_format.rs`
Cross-platform object-format adapter (ELF/PE/Mach-O). Picks the
right emitter per `target_triple`.

## Public types

- **`Pipeline`**  -  entry-point for a single `Program → object`
  compile.
- **`HostTu`**  -  translation-unit handle the host program holds
  while the compiled object is alive.
- **`ObjectFormat`**  -  enum of supported object formats.

## Integration points

- Consumes `vyre::ir::Program`.
- Shells out to `cc`/`clang`. The `.cargo-target-tools/` cache
  isolates the host-toolchain artefacts so they don't pollute
  vyre's main `target/`.
