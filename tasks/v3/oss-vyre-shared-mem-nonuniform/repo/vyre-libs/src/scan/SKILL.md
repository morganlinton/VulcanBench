# vyre-libs::matching SKILL

Byte/text scan primitives  -  substring search, DFA / Aho–Corasick. One ingredient inside larger vyre programs.
The DFA compilation produces a transition table as a u32 buffer; the
runtime Program walks the table one byte per step.

## Coverage targets

- `substring_search`  -  single-pattern brute-force match, one
  invocation per haystack offset.
- `aho_corasick`  -  multi-pattern scanner consuming a pre-built DFA.
- `dfa_compile` / `dfa_compile_with_budget`  -  CPU-side Aho-Corasick
  transition-table builder with size-budget enforcement.
- Future: `regex_compile`, `hyperscan_compat`, `simd_fixed_match`.

## Witness sources

- Substring search: simple corpus + edge cases (empty haystack,
  needle-larger-than-haystack, all-zeros, Unicode multi-byte). See
  `tests/cat_a_conform.rs` and `tests/aho_corasick_kat.rs`.
- Aho-Corasick: the 1975 paper's "ushers / he she his hers" example,
  20 hand-picked regression vectors, and the `aho-corasick` crate's
  test corpus.
- DFA budget: `tests/matching::dfa_compile::tests` exercises the
  `DfaCompileError::TooLarge` path.

## Benchmark targets (criterion)

- Substring search 4 KiB haystack × 3-byte needle: ≤ 50 µs CPU ref;
  dispatch backends ≤ 10 µs on current high-end fleet hardware.
- Aho-Corasick with 100 patterns × 4 KiB haystack: ≤ 1 ms CPU ref;
  dispatch backends ≤ 50 µs.

## DFA size contract

`dfa_compile` panics when the default 16 MiB budget is exceeded.
Structured-error callers use `dfa_compile_with_budget` and match on
`DfaCompileError::TooLarge`. See `tests/cat_a_conform.rs` for the
budget witness corpus.

## Overflow contract

The substring-search length guard (`needle_len <= haystack_len ∧
i + needle_len <= haystack_len`) is overflow-safe; see
`tests/cat_a_conform.rs::cat_a_substring_edge_cases` for the
needle-larger-than-haystack case.
