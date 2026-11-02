# F5 wave 1 sourcing: Rust candidates

Date: 2026-10-24. Sourcing only (no task building, no model runs).

Scope: upstream fix clusters in Rust crates for the F5 family (concentrated
multi-bug correctness) with the floor-8 property ("the obvious fix is wrong in
a way the project's own tests detect"). Window: merged 2026-06-01 or later,
preferring 2026-07 to 2026-10. Excluded by brief: petgraph, iOverlay, comrak,
and memory-safety or security-patch work.

Method: for 44 repositories, listed every merged PR since 2026-06-01 through
the GitHub GraphQL search API (`gh api graphql`, query
`repo:<r> is:pr is:merged merged:>=2026-06-01`), read PR bodies, review
threads and inline review comments with `gh pr view` and `gh api .../comments`,
and shallow-cloned the six leading repositories
(`--shallow-since=2026-05-01`) to read merge commits, diffs and test layouts.
Floor-8 evidence is judged from the public record only; nothing below has been
verified by running a naive patch. The authoring step must do that.

Dates are written as 2026-07-13 and ranges as "4 to 12". Merge SHAs are the
short hashes of the merge or squash commit on the default branch.

## (a) Ranked table

| Rank | Repo (licence) | Subsystem and files | Fixes (PR, merge SHA, merge date) | Regression tests added | Interaction evidence | Floor-8 evidence | Est. gold (source lines) | Est. expert hours | Build and test feasibility | Contamination note |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | [pulldown-cmark/pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) (MIT) | CommonMark block first pass and HTML/inline scanners: `pulldown-cmark/src/firstpass.rs` (3295 lines), `src/scanners.rs` (1679), `src/parse.rs` (3257) | Cluster A (HTML blocks and inline HTML): #1164 `def3889` 2026-10-08; #1165 `953e44a` 2026-10-09; #1163 `a3df587` 2026-10-12; #1169 `ad1622b` 2026-10-16; #1180 `044f471` 2026-10-19; plus #1116 `0424329` 2026-08-15. Cluster B (tabs and container prefixes): #1150 `9670b08`, #1151 `c80cc9f` (both 2026-09-28), #1159 `10e55b3` 2026-09-29, #1172 `5b0ef8c` 2026-10-07, #1173 `e465cac` 2026-10-12, #1170 `7e01b77` 2026-10-19, #1183 `862d8eb` 2026-10-23 | Mostly `specs/regression.txt` examples compiled into `tests/suite/regression.rs` (per maintainer rule in #1166); `tests/html.rs` for inputs without a trailing newline (#1164, #1157); `tests/errors.rs` for offset-range cases (#1168) | #1169's closing-tag fix "goes through the same newline handler" introduced by #1165; #1163 rebased on #1165 and #1157 after conflicts, and "#1167, #1169 and #1170 are rebased too" (comment on #1163); #1165 reuses `skip_container_prefixes` from #1002; #1151 reuses the #1143 offset formula; #1180 sits on #1172's `scan_containers` tab handling | Maintainer repeatedly rejected local fixes for structural ones: #1166 closed for #1172 ("The bug is actually in `scan_containers`"), #1177 closed for #1180 ("the code is needlessly complex"), #1134 superseded by #1183, #1124 by #1135, #1113 by #1116, #1145 by #1148; #1164 review: first version scanned the rest of the document on every line; #1169 review: "There is no test coverage for these five lines of code. I deleted them, and all of the tests still pass"; 652 CommonMark spec tests in `tests/suite/spec.rs` pin adjacent behaviour | Cluster A 150 to 250 (PR totals incl. tests and benches: #1163 +318/-12, #1164 +60/-5, #1165 +286/-35, #1169 +318/-12, #1180 +116/-14) | 4 to 8 for cluster A; 6 to 10 for A plus B | Cargo.lock committed; MSRV 1.70 (CI tests 1.71.1); pure Rust deps (bitflags, unicase, memchr, getopts, pulldown-cmark-escape; dev regex, serde_json, bincode); run `cargo test -p pulldown-cmark` (the `fuzz` workspace member pulls mozjs and bindgen, exclude it); suite runs in seconds | Fixes merged 2026-08 to 2026-10; PR bodies are long and public, several AI-assisted (Claude Code disclosures in #1150, #1151, #1159, #1144); v0.13.5 (#1181, 2026-10-21) ships most of cluster A |
| 2 | [apache/datafusion-sqlparser-rs](https://github.com/apache/datafusion-sqlparser-rs) (Apache-2.0) | `Display` round-trip of expressions and literals: `src/ast/mod.rs` (`Expr::UnaryOp` arm, `starts_with_operator_char`), `src/ast/value.rs`, `src/display_utils.rs`, `src/tokenizer.rs` (`OPERATOR_CHARS`) | #2531 `3fd342f` 2026-09-21; #2532 `7449e27` 2026-09-21; #2533 `0718147` 2026-09-21; #2538 `27da1cc` 2026-09-22; #2545 `d398304` 2026-09-22; #2544 `efafbc2` 2026-09-22; #2542 `80751ed` 2026-09-22; #2537 `2f029eb` 2026-09-22; #2541 `bfc7d38` 2026-10-01; #2543 `b1d1486` 2026-10-01; #2577 `3330dc9` 2026-10-19 | Each PR adds `verified_*` or `one_statement_parses_to` cases in `tests/sqlparser_common.rs` or `tests/sqlparser_postgres.rs`; #2577 adds `display_nested_unary_minus_in_linear_time` (5 s timeout) and `unary_prefix_operator_renders_apart_from_cast_operand`; #2541 adds nested `!` cases | A fix-of-a-fix chain in one match arm: #2541 replaced #2532's write-sink probe with an AST walk; #2577 replaced #2541's walk with a first-char probe that stops early and moved `OPERATOR_CHARS` into the tokenizer; #2533 and #2545 reuse the "spaced arm" that #2531 extended | #2577 body: #2541's walk "follows only unary and binary operators, so an operand that starts with an operator character under any other node is missed", and #2532's probe "doubled its cost with every nested minus, taking 172 ms for 22 signs" ("Mea culpa"); 3545 `verified_*` plus 460 `one_statement_parses_to` assertions pin canonical output, and #2531 and #2533 had to rewrite existing tests that asserted `SELECT {op}a` round-trips unchanged (removed lines visible in `3fd342f` and `0718147`) | 120 to 200 across the 11 PRs | 3 to 6 | Cargo.lock committed; pure Rust (log; optional serde, bigdecimal); dev simple_logger, matches, pretty_assertions; 1590 `#[test]` fns in 21 integration files; `cargo test` about 1 to 3 min | Fuzz-found (round-trip fuzzer from #2530); repo carries `AGENTS.md` ("Require a fuzz seed per bug fix", #2690); PRs 2026-09-21 to 2026-10-19 |
| 3 | [paupino/rust-decimal](https://github.com/paupino/rust-decimal) (MIT), `master` (2.0.0-alpha.0) | Scale and rounding arithmetic: `src/decimal.rs` (`round_sf_with_strategy`, `deserialize`, `trunc_with_scale`, `rescale`) and exact parsing in `src/str.rs` | #815 `5555207` 2026-07-17; #816 `6683079` 2026-07-19; #864 `0b97faa` 2026-10-06; #868 `b23fb07` 2026-10-08; #863 `cc04b8b` 2026-10-19; optional #827 `41f0e74` and #831 `c0a33e8` (both 2026-10-05) | `tests/decimal_tests/rounding.rs` (carry sweep, `round_sf_up_scale_stays_within_max_scale`), `parsing.rs` (midpoint tie cases; separator table), `scale.rs` (48 trunc combinations) | #815 and #816 are the `Less` and `Greater` branches of one function ("the up-scale sibling of #815's carry fix"); #863 and #864 both handle the scale 29 to 31 region that `rescale_internal` can produce; a generic cap in `rescale` conflicts with issue 618 behaviour (maintainer comment on #863) | #816: returning `None` "would regress that test" (`round_sf_large_digits_does_not_overflow`, present at base); #815: naive trailing-zero stripping breaks `("305.459", 7, Some("305.4590"))` (rounding.rs line 334 at base) and the doc test at `src/decimal.rs:1626`; #863: paupino "likely against merging it into master" for a generic cap, test asserts `rescale(40)` still yields scale 40, `issue_618_rescaling_overflow` relies on scale above 28; #868: 13 existing table rows pin `Err(Error::Underflow)` so an "accept any non-digit" fix changes error kinds | About 50 (#815 14, #816 9, #864 8, #863 2, #868 19) | 2 to 4 | No Cargo.lock on master (generate and vendor); default features alloc, serde, std; dev criterion, csv, serde_json, version-sync; `tests/generated` holds 251 MB of CSV fixtures in-repo; `cargo test --workspace --features=default` offline; skip db features; MSRV 1.85 | Fixes 2026-07 to 2026-10; master is a v2 alpha so crates.io v1 releases differ; several PRs AI-assisted |
| 4 | [unicode-rs/unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation) (MIT OR Apache-2.0) | `GraphemeCursor` chunked state machine: `src/grapheme.rs` (1196 lines, one file) | #175 `5dfedef` 2026-07-28; #180 `048d51f` 2026-09-02; #183 `e77d186` 2026-10-08 (plus #178 `091fb72` 2026-07-31 perf and #184 `346f36a` 2026-09-17 Unicode 18 tables in the same file) | `test_grapheme_cursor_ris_count_across_chunks` (#175, in `src/grapheme.rs`), `test_grapheme_cursor_prepend_before_control_on_chunk_start` plus `test_grapheme_cursor_chunked_matches_iterator` and `EXTRA_SAME` rows (#180, `tests/test.rs`); #183 adds no test ("every in-tree test passes unchanged") | #183 edits the `provide_context` entry that #180 changed ("The two `Prepend`-path guards" in the #183 thread); #175 and #180 both feed `cat_before`/state handling consumed by `check_pair` | #180: "the `InCbConsonant`, `Regional` and `Emoji` states ... reliance on the shortcut (needed, since `handle_emoji` and friends would otherwise mis-decide on a preceding `Prepend`) is untouched", so removing the shortcut breaks the pre-existing `test_grapheme_cursor_boundary_with_zwj_on_chunk_start`, `test_grapheme_cursor_emoji_no_zwj`, `test_grapheme_cursor_ris_precontext`; #175: resetting `ris_count` in `handle_regional` breaks the resumed backward walk pinned by `test_grapheme_cursor_ris_precontext` (PreContext then `Ok(true)`) | About 70 (#175 +23/-1, #180 +29/-1 in grapheme.rs, #183 +27/-5) | 2 to 4 | No Cargo.lock (generate); no runtime deps; dev quickcheck; test data in `tests/testdata/mod.rs` (219 KB, in repo); `cargo test` in seconds; MSRV 1.85 | #180 and #183 AI-assisted; code widely copied; three causes is the minimum and gold is small |
| 5 | [saphyr-rs/saphyr](https://github.com/saphyr-rs/saphyr) (MIT OR Apache-2.0) | YAML scanner and parser event production for empty nodes and stream boundaries, plus loader scalar resolution: `parser/src/scanner.rs` (2765), `parser/src/parser.rs` (1392), `saphyr/src/scalar.rs` (324); `saphyr/src/loader.rs` for #125 | #129 `eb4b66b` (merge `e7883a8` 2026-09-19); #136 `531e877` (merge `baaed40` 2026-10-11); #125 `05ee30f` 2026-10-09; #135 `2674517` (merge `e0f5876` 2026-10-15); #144 `2554cb6` 2026-10-23; #137 `097ce2f` 2026-10-23 (commits `5cd65d7` BOM, `bf52b46` %TAG, `744d5c4` spans); July: #93, #90, #92 | `parser/tests/issues.rs` (#144 `test_empty_block_scalar_at_eof_is_empty`, #135 four NUL cases), `saphyr/tests/basic.rs` (#136 `test_empty_node_is_null`), `parser/tests/bom.rs`, `tag_directives.rs`, `span.rs` (#137); vendored yaml-test-suite (351 cases) via `parser/tests/yaml-test-suite.rs` | #136 is required by #129 (fix of the fix: the parser change regressed the loader); #144 and #135 both change scanner stream-end handling; #137's BOM commit touches `input.rs` peeking that #135 changed (`peek_opt`) | #129 body: existing tests pinned the wrong behaviour in two places (`parser/tests/basic.rs:85` asserted `~`; `yaml-test-suite.rs:362` rewrote expected output `"=VAL :" => "=VAL :~"` with `// FIXME: known bug`) and the fix alone turned `a:\n` into `""` until #136; #136: "Treating every empty scalar as null, quoted ones included, fails its `''` assertion"; #144 keeps a Keep-chomping case ("An unterminated whitespace-only line still counts as a line"); #137: BOMs skipped only at document prefixes "and rejecting them elsewhere except quoted scalars"; #135: escaped `\0` in double-quoted scalars "is unaffected and pinned by a test" | 80 to 150 | 3 to 6 | Cargo.lock committed; pure Rust; yaml-test-suite vendored under `parser/tests/yaml-test-suite/src` and run with libtest-mimic; dev-deps miette, quickcheck, rustyline (pure Rust); MSRV 1.85; tests run in seconds | Fixes 2026-09 to 2026-10, released as saphyr 0.2.x with 0.1.x backports; #137 bundles four commits from one author |
| 6 (reserve) | [DioxusLabs/taffy](https://github.com/DioxusLabs/taffy) (MIT) | CSS Grid placement and track sizing: `src/compute/grid/mod.rs`, `implicit_grid.rs`, `explicit_grid.rs`, `types/named.rs`, `types/coordinates.rs` | #1167 `9ba171a0` 2026-08-25; #1169 `b3b38713` 2026-08-26; #1227 `ceb7443d` 2026-10-04; #1264 `7dfefc1f` 2026-10-08; #1258 `6ba55159` 2026-10-08; #1320 `63779e56` 2026-10-16 | Chrome-generated fixtures under `tests/xml/grid/*.xml` (four variants per case; 2340 grid fixtures, 6604 total), `tests/hand_written/detailed_grid_info.rs` | #1258 "Builds on #1264" and shares `find_line_index`; #1167 was extracted from the out-of-flow hoisting stack (#1194); #1227's two fixes are in adjacent blocks of `compute_grid_layout` | Weaker: #1258 rewrote three existing hand-written tests that pinned `i16::MAX` saturation ("the first test also covered a real bug"); #1227 shows a short-circuiting `any` with side effects, where the order-dependent result is pinned by two fixtures that differ only in child order; 6,597 fixture tests make any over-general placement change visible (unverified) | 250 to 400 | 6 to 12 | No Cargo.lock; pure Rust (roxmltree dev-dep, optional cssparser for the `parse` feature used by tests); `cargo test --workspace` about 6,600 tests in roughly a minute; MSRV 1.71 | Most PRs authored through Devin sessions with detailed public descriptions (links in PR bodies) |

## (b) Per-candidate notes: predicted naive fix and the guard it should break

### 1. pulldown-cmark (block first pass and HTML scanners)

Symptoms a task could present (all from the PR bodies): a `<script>` block
closed by `</SCRIPT>` swallows the rest of the document (#1163); `> <!-- x`
followed by `>` loses its last line (#1164); `> a <!--x\n> y--> b` renders the
`>` marker inside the comment (#1165); `a <![CDATA[x]y]]> b` renders as text
and `<a h=x\t)>` becomes an HTML block (#1169); `> \t<div>` turns the tab into
two spaces (#1180). Predicted naive fixes: (i) for #1163, lower-case the line
and search for the start tag's own end tag, which still misses the other three
tags, or end type-1 blocks at any `</` which breaks the CommonMark spec example
with `</code></pre>` on one line inside a `<pre>` block
(https://spec.commonmark.org/0.31.2/#html-blocks, type 1 examples in
`tests/suite/spec.rs`); (ii) for #1164, scan forward from every line to decide
whether only whitespace remains, which the maintainer rejected as a whole-input
scan per line and which also treats a real closing fence followed by blank
lines wrongly (PR text: tests include "a real end of input, a real closing
fence"); (iii) for #1165, strip the container prefix from every continuation
line of inline HTML, which breaks lazy continuation lines and lines that leave
the container (`> a <!X\n> > y`), both pinned by the PR's regression examples
and by the spec's lazy-continuation examples; (iv) for #1169, allow a newline
inside a closing tag without routing through the newline handler, which keeps
the container prefix and misses the "fewer than four spaces before `>` starts a
block quote" case the test checks, and the maintainer's added guard
`> a </t\n>     > b`. Floor-8 URLs:
https://github.com/pulldown-cmark/pulldown-cmark/pull/1166 (closed, "The bug is
actually in `scan_containers`"),
https://github.com/pulldown-cmark/pulldown-cmark/pull/1177 (closed for #1180),
https://github.com/pulldown-cmark/pulldown-cmark/pull/1134 (open, superseded by
#1183), https://github.com/pulldown-cmark/pulldown-cmark/pull/1124 (closed for
#1135), https://github.com/pulldown-cmark/pulldown-cmark/pull/1113 (closed for
#1116), https://github.com/pulldown-cmark/pulldown-cmark/pull/1145 (closed for
#1148). Hazard: #1159 changed two existing `tests/html.rs` expectations
(`newline_in_code`, `newline_within_code`) because they pinned non-spec
behaviour; keep #1159 out of the cluster or ship the updated expectations as
part of the task's hidden tests. Base suggestion: the commit before `def3889`
(2026-10-08) for cluster A, with #1116 and #1172 already in.

### 2. apache/datafusion-sqlparser-rs (Display round-trip)

Symptoms: `SELECT - -1` prints `SELECT --1` (a line comment), `SELECT ~ ~1`
prints `~~1`, `(a!)!` prints `a!!`, `SELECT X'ab''cd'` prints `X'ab'cd'`,
`REMOVE @```` prints with a bare backtick, `(1 . i)` prints `(1.i)`. Predicted
naive fix: add a space after every prefix operator (or after `-` whenever the
operand is a `UnaryOp`). The first form changes canonical output of `-1`, `-x`,
`+x` and fails the `verified_expr`/`verified_stmt` round-trip assertions that
dominate the suite (3545 uses), including the existing unary-op tables that
#2531 (`3fd342f`) and #2533 (`0718147`) had to rewrite. The second form is
exactly #2541's AST walk, which #2577 shows misses `-(-x)::INT` and `- @x`
(`unary_prefix_operator_renders_apart_from_cast_operand`), and the probe
variant of #2532 is exponential (`display_nested_unary_minus_in_linear_time`,
5 s timeout). Quote: "The rendering check from #2532 had no such stop and
doubled its cost with every nested minus, taking 172 ms for 22 signs"
(https://github.com/apache/datafusion-sqlparser-rs/pull/2577). Base
suggestion: the commit before `3fd342f` (2026-09-21); the gold is the state
after `3330dc9` restricted to the listed files. Note that the gold rewrites
existing canonical-output tests, so hidden tests must carry the post-fix
expectations.

### 3. rust-decimal (scale and rounding)

Symptoms: `dec!(0.95).round_sf(1)` gives `1.0` not `1`; `5e-27.round_sf(5)`
panics on `Display`; a deserialized scale-30 payload with coefficient 10 rounds
to `1e-28` instead of zero; `trunc_with_scale(40)` yields bytes that reload as
`10000.00000000`; `from_str_exact("0.1234567890123456789012345678_")` returns
`Underflow`. Predicted naive fixes and the guards: strip trailing zeros after
`round_sf` (breaks `("305.459", 7, Some("305.4590"))` in
`tests/decimal_tests/rounding.rs` and the doc test at `src/decimal.rs:1626`);
return `None` when the target scale exceeds 28 (breaks
`round_sf_large_digits_does_not_overflow`, quoted in
https://github.com/paupino/rust-decimal/pull/816); cap the scale inside
`rescale_internal` for everyone (maintainer objection in
https://github.com/paupino/rust-decimal/pull/863 and
`issue_618_rescaling_overflow` in `tests/decimal_tests/issues.rs:83`, plus the
#863 test asserting `rescale(40)` still has scale 40); accept any non-digit
tail in `from_str_exact` (changes the error kind pinned by the 13
`Err(Error::Underflow)` rows at `tests/decimal_tests/parsing.rs:264` to `279`
at the base). The #864 midpoint fix is independent and small (compare the
discarded remainder against 5, 50 or 500 by scale). Gold is tiny (about 50
lines), so this candidate is best used where the F5 slot wants a short gold
with long diagnosis; a `maths`-feature variant can add #827 (sqrt two-cycle)
and #831 (`to_f64` nearest).

### 4. unicode-segmentation (GraphemeCursor)

Symptoms: a cursor fed in chunks splits a flag (`"a\u{1F1E6}\u{1F1E6}"` gives
boundaries `[1, 5, 9]`); `\u{06DD}\r` queried across a chunk boundary returns
`Ok(false)`; out-of-range offsets panic inside `prev_boundary` and
`provide_context`. Predicted naive fixes: zero `ris_count` at the top of
`handle_regional`, which breaks the resumed backward walk pinned by the
pre-existing `test_grapheme_cursor_ris_precontext` (PreContext request, then
`Ok(true)`), while the gold only avoids re-entering the Regional walk when the
count is already known (`5dfedef`); delete the `Prepend` GB9b shortcut in
`provide_context`, which the #180 author says is still needed for the
`InCbConsonant`, `Regional` and `Emoji` states and so breaks the pre-existing
`test_grapheme_cursor_boundary_with_zwj_on_chunk_start`,
`test_grapheme_cursor_emoji_no_zwj` and RI tests (gold `048d51f` only skips
the shortcut when `cat_after` is Control, CR or LF); turn the `provide_context`
assertion into a hard `Err`, where the gold (`e77d186`) ignores a mismatched
call so a later correct call still works. Base suggestion: after `346f36a`
(Unicode 18, 2026-09-17) so test data matches, which means #175 is already in;
alternatively base before `5dfedef` with Unicode 17 tables and apply #175,
#180 (rebased) and #183. Three causes is the floor, and the cross-chunk API is
the only symptom surface.

### 5. saphyr (empty nodes, stream boundaries, loader resolution)

Symptoms: the parser emits `~` for an empty node while the YAML test suite
expects an empty scalar (38 cases); after fixing that, `a:\n` loads as `""`
instead of null; `a: |\n` at end of input yields `"\n"`; `key: a\0b` loads as
`key: a`; `\u{FEFF}\u{FEFF}a: 1` parses a key beginning with a BOM; only the
last `%TAG` directive is kept. Predicted naive fixes and guards: change
`empty_scalar()` to produce `""` and stop (this is exactly #129, which
regressed the loader, documented in
https://github.com/saphyr-rs/saphyr/pull/136); make every empty scalar null in
the loader (fails the `''` and `!!str` assertions in `test_empty_node_is_null`);
drop the header line break for every chomping mode in the block-scalar scanner
(fails the Keep case with an unterminated whitespace-only line, #144); treat
NUL as an error everywhere in the input layer (breaks the escaped `\0` in
double-quoted scalars that #135 pins); skip BOMs anywhere (contradicts spec
example 5.2 handled in `5cd65d7`). Note that #129 also removed the
`"=VAL :" => "=VAL :~"` rewrite in `parser/tests/yaml-test-suite.rs`, so a
task base before #129 has a test shim that must be removed for the vendored
suite to act as the guard. Base suggestion: the commit before `eb4b66b`
(2026-09-10), gold through `2554cb6` and `097ce2f` (2026-10-23) restricted to
scanner, parser, input and `saphyr/src/scalar.rs`.

### 6. taffy (reserve)

Symptoms: `grid-column: 3 / 1` and unknown named lines resolve differently from
Chrome for absolutely positioned children; `grid-column: auto / 1` on
`grid-template-columns: none` reports a phantom `0px` track; the same grid with
children in reverse order sizes columns differently; `span 2 col / 3` counts
from the wrong side. Predicted naive fixes: clamp placements to the explicit
grid before resolving (fails the new `grid_absolute_*` fixtures but no
pre-existing one, so weak); replace `any` with `all` or re-run track sizing
unconditionally (the latter changes results for fixtures whose first pass is
already final, unverified); special-case `span N name` by searching backwards
from the start line (the #1258 table shows four interlocking rules that must
agree). The clearest floor-8 fact is that #1258 had to change three existing
hand-written tests (`extreme_missing_named_line_indices_do_not_overflow`,
`large_named_span_does_not_wrap_negative`,
`area_lines_saturate_when_converted_to_grid_lines`), which shows the suite pins
adjacent behaviour, but the direction is the gold changing tests rather than
the naive fix failing them. Keep as a reserve if a large-gold layout task is
wanted.

## (c) Survey list

One line per repository: kept or rejected and why. Counts are merged PRs since
2026-06-01 from the GraphQL search.

- rust-lang/regex (15): rejected. One correctness cluster only: #1364 (2026-07-15, reverse suffix/inner optimisation, one root cause, +1592/-217) and #1366 (serialized DFA validation). The simpler #1355 was rejected for disabling the optimisation, which is a floor-8 flavour but with a single cause.
- winnow-rs/winnow (18): rejected. Only #928 (`seq!` with more fields) is a fix; the rest is chores.
- rust-bakery/nom (0): rejected. No merged PRs in the window.
- dtolnay/syn (72): rejected. Four `src/expr.rs` fixes (#2082 2026-09-16, #2085 2026-10-10, #2086 and #2087 2026-10-11, #2087 a follow-up to #2086) total about 15 lines, and `tests/repo/mod.rs` downloads a rust-lang/rust tarball at test time (`clone_rust`), which blocks offline testing.
- toml-rs/toml (49): rejected. Fixes are scattered across crates (#1189 writer overflow, #1194 datetimes in Value, #1211 `make_owned`, #1245 key encoding); no three-cause subsystem.
- clap-rs/clap (43): rejected. Fixes live in per-shell completion and mangen generators (#6414, #6409, #6468, #6526, #6457, #6482), different subsystems, snapshot tests but no floor-8 signal.
- chronotope/chrono (13): rejected. Two causes in `naive/date` (#1809 reversed iterators, #1810 `from_isoywd_opt`) plus unrelated tz fixes.
- time-rs/time (7): rejected. #782, #791, #815, #817 are in different modules.
- servo/rust-url (6): rejected. Two fixes (#1141 caret percent-encoding, #1127 file-URL hostname).
- unicode-rs/unicode-segmentation (7): kept, candidate 4.
- pulldown-cmark/pulldown-cmark (39): kept, candidate 1.
- georust/geo (39): rejected, borderline. The euclidean distance fast path has #1560 (2026-07-23, perf PR that also fixed a threshold overflow), #1603 (2026-09-01, fixes pruning unsoundness introduced by #1560; "The fact that our test suite didn't catch it but a proptest immediately did kinda gives me pause") and #1613 (2026-09-19, scale invariance). The #1603 bug does not exist at a base before #1560, so only two independent causes compose; the build also pulls i_overlay and spade.
- paupino/rust-decimal (30): kept, candidate 3.
- rust-num/num-bigint (18): rejected. Burnikel-Ziegler #348 (missing step, fuzz-found) and #350 (algorithm 3) are one algorithm in two PRs; #355 is unrelated.
- RoaringBitmap/roaring-rs (6): rejected. #360 (treemap `advance_to`, four sub-cases), #363 (interval store off-by-one), #389 (empty partitions in treemap cmp) span `treemap/iter.rs`, `bitmap/store/interval_store.rs` and `treemap/cmp.rs`; two treemap causes only.
- pest-parser/pest (32): rejected, borderline. Validator pair #1211 (2026-10-08) and #1227 (2026-10-12) in `meta/src/validator.rs`, parser-state pair #1180 and #1184; each pair is two causes, and #1227 is a +1095-line analysis pass, more feature than fix.
- lalrpop/lalrpop (11): rejected. No correctness fixes (#1139 is a build-script rebuild trigger).
- tafia/quick-xml (23): rejected. The NamespaceResolver changes #972, #979 and #987 are resource limits (OOM and depth hardening), an excluded class; the semantic fixes #1002 (serde namespace timing), #1024 (reserved default namespace), #1001 (escaping) and #1023 (decoding prefix) are spread across `de/`, `name.rs`, `se/` and `reader/`.
- BurntSushi/rust-csv (0): rejected. No merged PRs.
- cessen/ropey (0): rejected. No merged PRs.
- image-rs/image (46): rejected, borderline. ICO decoder cluster of eight PRs 2026-06-03 to 2026-06-28 (#3002, #3001, #3027, #3035, #3032, #3056, #3003, #3045) with follow-ups (#3035 after #3002, #3056 after #3032) and maintainer pushback ("Could you please show that this is correct?"; "lenient spec compliance should accept more inputs, not change what accepted inputs get decoded as"). Rejected because the fixes are leniency policy rather than algorithmic correctness, the window is June, reference PNGs must be regenerated, and default features pull heavy codec dependencies.
- linebender/resvg (51): rejected. usvg fixes (#1084, #1080, #1085, #1135, #1164, #1163, #1144, #1155) span paint servers, transforms, markers and filters; PNG reference tests are feasible but the subsystem is spread and text tests depend on fonts.
- DioxusLabs/taffy (223): kept as reserve, candidate 6.
- pop-os/cosmic-text (5): rejected. One fix (#508).
- harfbuzz/rustybuzz (0): rejected. No merged PRs.
- dfrg/swash (8): rejected. Two `HintingCache` fixes (#114, #121) in June.
- apache/datafusion-sqlparser-rs (190): kept, candidate 2. Other clusters worth a second look: SQLite precedence (#2683 2026-10-20, #2678 2026-10-23, #2657 2026-10-24, with #2576 for MySQL `&&`) and string-literal aliases (#2668 2026-10-18, #2669 2026-10-21, #2682 2026-10-23).
- BurntSushi/jiff (34): rejected. Fixes scattered (#614 fmt sign, #617 tz offset subtraction, #628 serde Option, #658 duration buffers).
- saphyr-rs/saphyr (21): kept, candidate 5.
- servo/html5ever (22): rejected. Fixes are memory-safety (#754, #755, #756) or single (#764, #779, #780 in xml5ever, #784).
- ratatui/ratatui (182): rejected, borderline. Wide-glyph buffer-diff cluster #2696 (2026-08-22), #2697 (2026-08-23, "The first version was wrong in both directions: it broke wide glyphs"), #2721 (2026-08-24), #2735 (follow-up), #2743 (2026-09-03) spans `ratatui-core/src/buffer/diff.rs` and three backend crates; the October clipping PRs (#2815 to #2821) are one pattern repeated per widget.
- rust-itertools/itertools (11): rejected. No correctness cluster.
- linebender/parley (221): rejected. Many line-breaking fixes (#805, #815, #819, #829, #834, #842, #844, #850, #853, #854, #863, #864) but font-dependent, heavy deps (icu, harfrust), snapshot tests.
- kdl-org/kdl-rs (13): rejected, borderline. serde `src/de.rs` fixes #183 (2026-09-17), #185 (2026-09-23, case missed by #183), #197 (2026-10-15) are three causes in one file, but the maintainer accepted a "kinda ugly" fix and nothing shows the obvious fix failing existing tests.
- rust-lang/annotate-snippets (0): rejected. No merged PRs.
- rust-lang/mdBook (65): rejected. Fixes are in the HTML/JS renderer and preprocessor; the include-handling fixes are in one PR (#3196).
- image-rs/image-png (12): rejected. APNG encoder #708 and #703, decoder #718; two causes per area.
- bytecodealliance/wasm-tools (216): not fully assessed, promising for wave 2. Component validation conformance fixes by alexcrichton 2026-09-28 to 2026-10-06 (#2692 table64, #2691 async with memory64, #2727 borrows in streams, #2741 tags, #2750 immutable-global subtyping, #2752 kebab-name type checks, #2753 named imports) all sit in `crates/wasmparser/src/validator/component_types.rs` and `component.rs`. Apache-2.0 WITH LLVM-exception; `.wast` snapshot tests are in-repo but round-trip tests need the testsuite submodule and the workspace build is heavy; no floor-8 evidence found in PR texts.
- rust-lang/rustfmt (110): rejected. Requires a nightly toolchain (rustc internal crates).
- sunng87/handlebars-rust (26), Keats/tera (22), getzola/zola (55): rejected. One to four scattered fixes each.
- linebender/kurbo (5): rejected. No fixes.
- Byron/gitoxide: search returned 0 merged PRs (likely merged outside the PR flow); not assessed.

## (d) Gaps and uncertainties

- Floor-8 claims are predictions from PR bodies, review threads and the
  pre-fix test files. None was verified by applying a naive patch and running
  the suite. The authoring step must do that for every cause it keeps, and the
  predicted naive fixes above are the ones to try first.
- pulldown-cmark: cluster A and cluster B overlap in `firstpass.rs`; choosing a
  base inside the September to October run means some fixes from the other
  cluster are already in or still missing. #1164's tests cannot live in
  `regression.txt` (inputs without a trailing newline), so hidden tests for it
  must be hand-written. #1159 changed existing expectations.
- sqlparser-rs: the gold rewrites existing canonical-output tests, so hidden
  tests are the post-fix `verified_*` expectations; the task must not also
  keep the pre-fix assertions. `rust-version` was not seen in `Cargo.toml` at
  HEAD (`594bb04`), so confirm the stable toolchain CI uses.
- rust-decimal: `master` is 2.0.0-alpha.0 with no committed Cargo.lock; the
  gold is about 50 lines, which may fall below the F5 size target; the 251 MB
  of generated CSV fixtures inflate the image.
- unicode-segmentation: no Cargo.lock; #184 (Unicode 18, 2026-09-17) sits
  between #180 and #183 and regenerates tables and test data, so the base must
  be chosen on one side of it; only three causes.
- saphyr: #129 removed a test-suite expectation shim; a base before #129
  carries that shim, which hides the bug from the vendored suite until the
  shim is removed. #137 bundles four commits, two of which (docs, spans) are
  outside the cluster.
- taffy: large Devin-authored PRs; floor-8 evidence is indirect.
- Build and test times were not measured in Docker; estimates come from
  dependency lists and PR descriptions (pulldown-cmark, saphyr and
  unicode-segmentation run in seconds; sqlparser-rs and taffy in one to three
  minutes; rust-decimal a few minutes with generated fixtures).
- Contamination: every candidate's fixes were merged between 2026-07 and
  2026-10 with detailed public PR bodies, many AI-assisted. They post-date a
  June 2026 training cutoff, but models trained later will have seen them.
- Not surveyed this wave: typst, oxc, biome, ruff, swc, gitoxide (large or
  slow builds); wasm-tools only partially.
