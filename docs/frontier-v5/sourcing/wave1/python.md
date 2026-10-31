# F5 wave 1 sourcing: Python candidates

Date: 2026-10-24. Sourcing only (no task building, no model runs).

Target: upstream fix clusters for F5 (concentrated multi-bug correctness) with
floor 8, "the obvious fix is wrong in a way the project's own tests detect".
Window: fixes merged 2026-06-01 or later, preferring 2026-07 to 2026-10.
Excluded up front: NetworkX, SymPy, memory-safety and security patches.

Method: counted merged PRs since 2026-06-01 for about 70 repositories with
`gh pr list` and the search API, then shallow-cloned the promising ones
(`--shallow-since=2026-05-01`) into the scratchpad and read per-module `git
log`, PR bodies, review threads and the test diffs. Test-suite timings below
were measured locally on macOS arm64 with Python 3.12.15 in `uv` venvs; the
authoring step still has to confirm them in the Docker image. Every claim
carries a PR or commit id; PR URLs are `https://github.com/<repo>/pull/<n>`.

## (a) Ranked candidates

Summary table. Full fix lists, tests and quotes follow in the per-candidate
sections (C1 to C6), which are part of this section.

| Rank | Repo (licence) | Subsystem and files | Fixes in cluster (merge dates) | Interaction | Floor 8 | Gold lines (source only) | Expert hours | Build and test | Contamination |
|---|---|---|---|---|---|---|---|---|---|
| C1 | psf/black (MIT) | `# fmt: skip` handling: `src/black/comments.py` (walk in `_generate_ignored_nodes_from_fmt_skip`, `stringify_node`, `_get_compound_statement_header`) plus two spots in `src/black/linegen.py` | #5401, #5405, #5429, #5464, #5522, #5538, #5584, #5586, #5588, #5597 (2026-09-22 to 2026-10-24) | strong: fixes amend each other (#5538 extends #5405, #5597 extends #5401, #5584 extends #5522, #5549 repairs #5401) | strong: three fixes were regressions of earlier fixes; every case file is checked for idempotence and AST equivalence | +155 -28 for the ten PRs; a 5-fix subset is about +90 -20 | 6 to 12 | pure Python, 628 tests in 14 s wall (`-n 4`) | high: detailed public PR write-ups, several explicitly written with Claude Code |
| C2 | python-poetry/tomlkit (MIT) | out-of-order tables, super tables and dotted keys: `tomlkit/container.py` (`Container.append`, `_validate_table_candidate`, `_replace_at`, `_get_last_index_before_table`, `OutOfOrderTableProxy`) | #516, #530, #565, #526, #549, #550, #545, #536, #498, #507 (2026-06-15 to 2026-10-12) | strong: #565 corrects #516/#530, #549 extends #526's guard, #530 builds on #509 | strong: #565 is an over-rejection introduced by the earlier fixes; two existing tests pin behaviour the obvious fix changes (#567 says so explicitly) | +158 -11 for the ten PRs; a 6-fix subset is about +80 -10 | 5 to 10 | pure Python, 1203 tests in 0.9 s; needs the `tests/toml-test` submodule vendored | high: public PR write-ups, two PRs written with Claude Code |
| C3 | tobymao/sqlglot (MIT) | correlated subquery decorrelation: `sqlglot/optimizer/unnest_subqueries.py` (`decorrelate`, `_has_aggregate_projection`), with `optimizer/helpers.py` for #8381 | #8236, #8304, #8317, #8318, #8381, #8589, #8638, #8742, #8730 (2026-08-21 to 2026-10-23) | strong: #8318 came out of the review of #8317; #8381 rewrites #8304's helper; #8742 constrains #8638 | strong: #8742 corrected an existing fixture that pinned wrong output; the naive "skip JOIN ON" fix breaks #8638's fixtures; `execute: true` fixtures are checked against DuckDB | +296 -73 for nine PRs; a 5-fix subset is about +250 -70 | 8 to 16 | pure Python; `tests/test_optimizer.py` + `tests/test_executor.py` need duckdb and pandas (wheels exist), 181 tests and 5432 subtests in 48 s | high: `[CLAUDE]`-tagged PRs with root-cause write-ups and a public integration-test site |
| C4 | pyparsing/pyparsing (MIT) | `ParseResults` results-name bookkeeping and arithmetic: `pyparsing/results.py` (`copy`, `deepcopy`, `insert`, `__delitem__`, `__add__`, `__iadd__`, `__radd__`, `as_dict`) plus `Dict.postParse` in `pyparsing/core.py` | #642, #643, #648, #645 plus maintainer follow-ups 6f2fbcb and a6ae98b (2026-06-24 to 2026-08-09) | medium: #648 is the `__iadd__` twin of #642 and `__add__` calls `copy()` then `+=`; 6f2fbcb moved #645's copy into the mutators | medium: `testParseResultsCopy` pins shallow sharing, three existing `sum(expr.search_string(...))` tests pin `0 + ParseResults`, pickle tests pin `__getstate__`/`__setstate__` | +92 -38 | 3 to 6 | pure Python, no deps; `tests/test_unit.py` 1952 tests in 23 s | medium: PR bodies are detailed; two PRs written with Claude |
| C5 | pallets-eco/croniter (MIT) | `expand_from_start_time` phase and range expansion: `src/croniter/croniter.py` (`_get_low_from_current_date_number`, `_expand`, `HashExpander`) | #235, #239, #246, #253, #254, #256 (2026-07-02 to 2026-08-01) | strong: #253 "found while reviewing #246"; #256 found by "an adversarial pass over #246/#253/#254/#255" | medium: #246 must keep explicit equal ranges expanding to the whole cycle (pinned by tests and the changelog); #256 must leave naive start times unchanged (sweep of 45,312 expansions) | +79 -24 | 3 to 6 | pure Python, deps python-dateutil and pytz; about 230 tests, seconds | medium: public PR write-ups; #246 from an automated pipeline |
| C6 | hgrecco/pint (BSD-3-Clause) | `wraps()` and `check()` decorators: `pint/registry_helpers.py` | #2325, #2336, #2422, #2426 (plus typing-only #2460) (2026-07-26 to 2026-10-12) | medium: #2422 builds on #2336's `**kwargs` handling and keeps its trailing-`None` compatibility; #2426 mirrors #2422's binding for `check()` | weak to medium: existing `test_wraps`/`test_check` pin strict-mode `TypeError`s and the `None` placeholder; review friction was performance (codspeed), not failing tests | +162 -57 | 4 to 8 | pure Python; numpy needed for most of the suite; 2786 tests in 41 s excluding `pint/testsuite/benchmarks` | medium: PR bodies detailed; #2422 and #2460 written with Codex |

### C1. black: `# fmt: skip` crash and fidelity cluster

Repo: https://github.com/psf/black (MIT). Files: `src/black/comments.py`
(1124 lines), `src/black/linegen.py` (2903 lines). Test harness:
`tests/test_format.py` runs every `tests/data/cases/*.py` through
`assert_format`, which calls `black.assert_equivalent` (AST) and
`black.assert_stable` (second pass) (`tests/util.py` lines 182 to 183).
There are 49 `fmtskip*`/`fmtonoff*` case files on main today.

Fixes (merged, source lines, regression test added):

- #5405 (2026-09-22) `_force_standalone_comment_split` must not split before
  the header colon after a skipped bracket; `linegen.py` +6 -2;
  `tests/data/cases/fmtskip_header_bracket_5402.py`. Body: "Fixes an
  unreleased regression" (the regression came from #5144, 2026-05-28).
- #5401 (2026-09-24) keep paired `# fmt: skip` comments on a compound header
  adjacent; `comments.py` +46 -8; `fmtskip_paired_header.py`. Review by
  torjan0 found the nested `stringify_node` branch still reformatting skipped
  content, and the fix was widened before merge.
- #5429 (2026-09-29) climb out of fully ignored `typeparams`/`parameters`
  nodes instead of leaving empty nodes behind; `comments.py` +15 -0;
  `fmtskip_type_params.py`. Body: "It's a regression from #4552".
- #5464 (2026-10-01) keep climbing while the parent is fully skipped so a
  bracketed ternary keeps its comma; `comments.py` +28 -9;
  `fmtskip_conditional_in_brackets.py`.
- #5522 (2026-10-22) widen a closing bracket to its call or subscript
  trailer; `comments.py` +1 -1; `fmtskip_after_multiline_call.py`.
- #5538 (2026-10-22) keep the leaves after a converted standalone comment on
  the same line when its last line has no `#`; `linegen.py` +18 -0;
  `fmtskip_opening_bracket_split.py`. Body: "Bisects to #5144 ... #5405 fixed
  the same split for a compound header's `:` (#5402); this covers the rest of
  the line."
- #5584 (2026-10-23) widen the closing parenthesis of an `import_from`;
  `comments.py` +5 -1; `fmtskip_after_multiline_import.py`. Body: "like the
  call in #5522".
- #5586 (2026-10-23) leave out the invisible parentheses `visit_test` adds
  when the first operand is hidden; `linegen.py` +11 -1;
  `fmtskip_opening_bracket_conditional.py`. Body: "Inside brackets they're
  kept, so existing output there doesn't change."
- #5588 (2026-10-23) only prepend header leaves the walk has not covered;
  `comments.py` +20 -5; `fmtskip_one_liner_bare_name.py`.
- #5597 (2026-10-24) only append the newline after a standalone comment when
  its last line has a `#`; `comments.py` +5 -1; three cases added to
  `fmtskip_paired_header.py`. Body: "A header where the inner `)` also has
  `# fmt: skip` works; that is the case #5401 fixed."
- Related, same files, not counted above: #5549 (2026-10-23) "The `# fmt:
  skip` case started in 26.10.0 with #5401" (a regression caused by #5401),
  #5585 (2026-10-22) fmt: off comments inside brackets, #5311 (2026-08-19)
  async one-liners, #5161 (2026-06-08) `bracket_depth` crash.

Interaction evidence: the fixes share one backward walk and one serializer.
#5538 and #5586 both handle code that follows a hidden closing bracket on the
same line, through two different paths (#5586: "This doesn't reach the
`_force_standalone_comment_split` path from #5538, because the split succeeds
first"). #5597 depends on #5401's sibling handling. #5584 reuses #5522's
widening. #5588 depends on #4800's header prepending and #5311's async
handling.

Floor 8 evidence: three of the merged fixes were regressions of earlier
fixes (#5405 from #5144, #5429 from #4552, #5549 from #5401), i.e. upstream
itself shipped "obvious" fixes that broke adjacent cases. The suite catches
such breakage because every case file is checked for stability and AST
equivalence, and the `fmtskip*` files pin that only the directive's own line
is left verbatim while the rest of the collection is still formatted
(#5522: "just that element is left as is and the rest of the collection is
still formatted"). URLs: https://github.com/psf/black/pull/5538,
https://github.com/psf/black/pull/5549, https://github.com/psf/black/pull/5597.

Build and test: `pip install -e .` is pure Python (mypyc wheels are optional);
runtime deps click, mypy_extensions, packaging, pathspec, platformdirs,
pytokens; test deps pytest, pytest-xdist; `aiohttp` only for blackd tests.
Measured 628 passed, 3 skipped in 14 s wall with `-n 4`.

Contamination: PR bodies carry root-cause analyses; #5584, #5588, #5597,
#5538 and #5585 state they were written with Claude Code (Opus 5.5). The
project runs diff-shades on every PR. Suggested base: main at 2026-10-21
(before #5522), with the five fixes #5522, #5538, #5584, #5586, #5588 and
#5597 as gold; or base at 2026-09-21 for a wider set.

### C2. tomlkit: out-of-order tables, super tables and dotted keys

Repo: https://github.com/python-poetry/tomlkit (MIT). File:
`tomlkit/container.py` (1362 lines). Tests: `tests/test_toml_document.py`
(1806 lines, 27 out-of-order and redefinition tests), `tests/test_parser.py`,
`tests/test_api.py`, and `tests/test_toml_tests.py`, which runs the vendored
BurntSushi toml-test suite (`tests/toml-test` submodule, 894 case files
listed in `files-toml-1.1.0`).

Fixes (merged, `container.py` lines, regression test):

- #509 (2026-06-15, binggao1230) validate out-of-order tables incrementally
  while parsing; +63 -4; `tests/test_api.py::test_parse_many_dotted_keys_with_shared_prefix`,
  `test_parse_rejects_collisions_across_out_of_order_fragments`. Perf fix
  that the later validation fixes build on.
- #516 (2026-06-15) reject a table redefined after its parent header
  (`[a.b]` / `[a]` / `[a.b]`); +8 -0;
  `tests/test_parser.py::test_parser_rejects_table_redefined_after_parent`.
  Maintainer frostming asked why toml-test did not catch it; the author
  showed the suite has no case of that shape.
- #530 (2026-06-23) add the missing `else` branch in `Container.append` so a
  concrete table extended by a super table after an unrelated header is
  validated at parse time, and detect a non-dotted candidate that is a prefix
  of an existing dotted key; +18 -0; tests
  `test_reject_out_of_order_dotted_key_redefinition_at_parse`,
  `test_reject_out_of_order_dotted_prefix_at_parse`,
  `test_valid_out_of_order_independent_tables` (a guard added with the fix).
  It also rewrote `test_unwrap_preserves_raise_on_invalid_out_of_order_fragment`
  from `KeyAlreadyPresent` on access to `ParseError` at parse.
- #565 (2026-10-12) do not reject an out-of-order super table extended by a
  dotted key; +3 -1; `test_valid_out_of_order_super_table_extended_by_dotted_key`.
  This repairs an over-rejection introduced by #516/#530.
- #526 (2026-06-23) replacing a dotted key with a table: drop the dotted key
  and move the new header past inline siblings; +32 -3; four tests
  (`test_replace_dotted_key_with_table`, `..._with_empty_table_keeps_following_sibling`,
  `..._with_table_keeps_following_sibling`, `test_replace_value_with_table_keeps_following_dotted_sibling`).
- #549 (2026-07-14) extend #526's `dotted_to_header` guard to `AoT`; +7 -5;
  `test_replace_dotted_key_with_aot_keeps_following_sibling`. Body: "the
  table case (#513/#524) never ran for arrays of tables."
- #550 (2026-07-15) `_get_last_index_before_table` stops at a dotted-key
  table whose children render a header; +24 -0;
  `test_scalar_is_not_captured_by_table_rendered_from_dotted_key`.
- #545 (2026-07-14) route a plain value into the existing concrete part of an
  out-of-order table instead of writing a second `[x]` header; +12 -1;
  `test_set_value_on_out_of_order_table_with_empty_concrete_part`.
- #536 (2026-07-14) proxy repr uses the merged container; +6 -0;
  `test_repr_out_of_order_table_proxy`.
- #498 (2026-06-23) a sub-table header naming an existing array of tables
  merges into the last element; +10 -0; `test_subtable_of_aot_element_after_other_table`.
- #507 (2026-06-17) merge AoT fragments when building the proxy; +38 -1;
  `test_out_of_order_table_merges_aot_fragments` and the three-fragment test.
- Adjacent, same file, October: #591 (2026-09-30) unwrap() order after a
  replacement, #567 (2026-10-13) render an emptied array of tables as `[]`.

Interaction evidence: #565's whole diff is a reordering inside
`_validate_table_candidate`, the function #516 and #530 built ("Fix: only
take the dotted-key rejection when the existing entry is not a super-table;
otherwise fall through to the existing recursion"). #549 is a one-line
extension of #526's guard. #530 calls #509's `_validate_out_of_order_table`
from the fallthrough path. #550 and #526 both reason about which body items
render inline.

Floor 8 evidence:

- Upstream shipped the obvious rejection and it broke valid TOML: #565,
  "tomlkit rejects valid TOML on the `parse()` path when an out-of-order
  table reopens an implicit super-table with a dotted key ... The dotted
  rejection is only correct when the existing side is a concrete table."
  https://github.com/python-poetry/tomlkit/pull/565
- Existing tests pin behaviour the obvious rendering fix would change: #567,
  "the current behaviour is pinned by two existing tests,
  `test_delete_out_of_order_table_key` and `test_overwriting_out_of_order_table`
  ... Making emptied super tables render `[x.y]` breaks both."
  https://github.com/python-poetry/tomlkit/pull/567
- #550 keeps the inline case unchanged: "Purely inline dotted keys are
  unaffected (`a.b = 1` then `doc["z"] = 2` still appends `z = 2` after the
  dotted line)", which `test_toml_document_with_dotted_keys` and
  `test_add_dotted_key` (both pre-existing) pin.
- The toml-test valid suite (hundreds of files) runs on every parse change,
  so any rejection rule that is too wide fails there.

Build and test: pure Python, no runtime dependencies; test deps pytest and
pyyaml; the `tests/toml-test` submodule must be vendored at image build
(`git submodule update --init --depth 1`). Measured 1203 passed in 0.9 s.

Contamination: PR bodies are detailed; #565 and #550 state they were written
with Claude Code. Many issue numbers (#513, #523, #524, #542, #543) describe
the symptoms publicly. Suggested base: 2026-06-15 right after #509, with
#516, #530, #565, #526, #549, #550 (and optionally #545) as gold; #565 then
becomes the guard against the over-rejection the base's naive fix produces.

### C3. sqlglot: correlated subquery decorrelation (`unnest_subqueries`)

Repo: https://github.com/tobymao/sqlglot (MIT). File:
`sqlglot/optimizer/unnest_subqueries.py` (494 lines); #8381 also touches
`sqlglot/optimizer/helpers.py` and `pushdown_projections.py`, #8730 touches
`generator.py`. Tests: `tests/fixtures/optimizer/unnest_subqueries.sql` (376
lines, 15 fixtures marked `execute: true` are run through the Python executor
and DuckDB), `tests/fixtures/optimizer/optimizer.sql`, `tests/test_executor.py`
(`test_correlated_not_in_is_not_unnested`, `test_correlated_count`,
`test_correlated_exists_over_scalar_aggregate`, `..._nested_scalar_aggregate`).

Fixes (merged, source lines, tests):

- #8236 (2026-08-21) decline decorrelation of a negated correlated `IN`;
  +11 -1; fixture plus 20 lines in `test_executor.py`. Author jbylund:
  "There are a few other decorrelate rewrites that are not necessarily safe."
- #8304 (2026-09-07) `EXISTS` over a scalar aggregate folds to `TRUE`, except
  under `HAVING`/`QUALIFY`, windows, nesting or set operations; +45 -5; 67
  fixture lines and 104 executor-test lines.
- #8317 (2026-09-08, maintainer georgesittas) decorrelation when non-EQ keys
  are present; +67 -32; 36 fixture lines including two fixtures whose
  expected output changed because the old output returned wrong rows.
- #8318 (2026-09-08) do not fan out `EXISTS` decorrelation over grouped
  subqueries; +28 -3; 52 fixture lines. Body: "See also:
  https://github.com/tobymao/sqlglot/pull/8317#discussion_r3958781681".
- #8381 (2026-09-24) windowed aggregates do not force aggregation;
  `_has_aggregate_projection` now uses the new `projection_has_aggregate`
  helper; +79 -27 across three files.
- #8589 (2026-10-12) apply the alias after the `COALESCE` wrap; +3 -3;
  fixture in `optimizer.sql` (`execute: false` because of column naming).
- #8638 (2026-10-15) place the generated `LEFT JOIN` before the join whose
  `ON` references it; +29 -0; tests live in the `sqlglot-integration-tests`
  submodule only.
- #8742 (2026-10-22) skip unsafe rewrites in `JOIN ... ON` (LEFT/RIGHT/FULL/
  SEMI/ANTI, `OR`, `NOT ANY`) while keeping inner-join rewrites; +27 -1; 79
  fixture lines, 26 lines in `test_optimizer.py`, and "correct an existing
  fixture that expected the row-dropping output" in `optimizer.sql`.
  Fixes issue #8737.
- #8730 (2026-10-23) `GROUP BY <int>` emitted by unnesting is an ordinal, so
  emit `0 + 0`; +7 -1 (also `generator.py`).

Interaction evidence: the review of #8317 produced #8318 (review comment on
`unnest_subqueries.py` line 231: "A non-`EQ` predicate whose key happens to
match the projection would be routed to the `GROUP BY` path and then silently
dropped"). #8381 replaces the aggregate check #8304 introduced. #8742 limits
the join placement #8638 added one week earlier. #8304's folding and #8318's
grouping rules both decide whether a subquery can return zero rows.

Floor 8 evidence:

- The obvious fix for issue #8737 (never unnest inside `JOIN ON`) would fail
  the inner-join fixtures that #8638 and #8742 keep ("retaining supported
  inner join transformations"), and the pre-#8742 `optimizer.sql` fixture
  pinned the wrong, row-dropping output, so the task base must drop or
  correct that fixture. https://github.com/tobymao/sqlglot/pull/8742
- The obvious fix for the `EXISTS` aggregate bug (fold whenever the
  projection contains an `AggFunc`) is what #8304 shipped and #8381 had to
  amend for `COUNT(*) OVER ()`; the `HAVING`/`QUALIFY` fixtures and
  `test_correlated_exists_over_scalar_aggregate` pin the non-folded cases.
  https://github.com/tobymao/sqlglot/pull/8381
- The obvious fix for #8317 (refuse decorrelation unless all keys are EQ)
  breaks the existing `<> ANY` and `ARRAY_AGG` fixtures at the top of
  `unnest_subqueries.sql`, which expect those shapes to be rewritten.
- Several fixtures carry `execute: true`, so a rewrite that parses but
  returns different rows fails against DuckDB.

Build and test: pure Python (sqlglotrs/sqlglotc are optional); test deps for
the optimizer and executor: duckdb (2.0.0 wheels for arm64 and x86-64),
pandas, python-dateutil. Measured `tests/test_optimizer.py` plus
`tests/test_executor.py`: 181 tests, 5432 subtests, 48 s. The full suite also
wants pytz and must skip `tests/test_sqlglotc.py`. Many fixes carry `!`
(behaviour change) in their titles, which is fine for a frozen base.

Contamination: PR titles are tagged `[CLAUDE]`/`[CODEX]`, bodies contain
before/after tables, and a public integration-test site compares every PR.
Suggested base: 2026-09-06 (before #8304) with #8304, #8317, #8318, #8381
(helpers part), #8742 as gold, or 2026-10-11 with #8589, #8638, #8742, #8730.

### C4. pyparsing: `ParseResults` bookkeeping and arithmetic

Repo: https://github.com/pyparsing/pyparsing (MIT). File:
`pyparsing/results.py` (981 lines); the `Dict` fix also touches
`pyparsing/core.py`. Tests: `tests/test_unit.py` (12132 lines, one file).

Fixes (merged, source lines, tests):

- #642 (2026-06-30, commit 16f3486) `__add__` returns `NotImplemented` for
  non-`ParseResults`, `__radd__` no longer recurses; +3 -3;
  `testParseResultsArithmeticContract`.
- #643 (2026-07-15, 92f52e2, simplified by ptmcg in b4175cf) `deepcopy()`
  rebuilds `_tokdict` so names point at the copied tokens; +28 -1 then
  +10 -19; test at `test_unit.py` line 4531 ("deep copy failed for named
  result").
- #648 (2026-08-01, b0e2777) `__iadd__` guard, the twin of #642; +2 -0;
  cases added to the contract test. Body notes `pr += []` now raises like
  `pr + []`.
- #645 (2026-08-09, 94842d0) `copy()` must not share the per-name occurrence
  lists; +3 -1; `testParseResultsCopyResultsNamesAreIndependent`. The
  maintainer asked for the cost to move off the parse path, and 6f2fbcb
  (2026-08-06, +14 -7) moved the copying into `insert` and `__delitem__`.
- a6ae98b (2026-08-09, maintainer, adopting PR #635) `Dict` returns `{}` not
  `[]` for an empty nested result via a new `_is_dict_context` slot carried
  through `__getstate__`/`__setstate__`; +32 -7 across `results.py` and
  `core.py`; `testEmptyDictReturnsDictWhenCallingAsDict` and a pickle
  attribute sweep.

Interaction evidence: #648 body: "This is the `__iadd__` counterpart of
#642"; `__add__` is `ret = self.copy(); ret += other`, so #642, #645 and
#648 meet in one expression. #645 body: "This is the same seam as
#642/#643." The maintainer's rework of #643 (`memo.get(id(value), value)`)
is what #645's deepcopy comparison relies on.

Floor 8 evidence:

- `testParseResultsCopy` (pre-existing, line 4332) asserts
  `r2[1] is result[1]` ("shallow copy failed"), so the obvious fix for #645
  (deep-copy everything in `copy()`) fails it; #645's body cites exactly this
  guard. https://github.com/pyparsing/pyparsing/pull/645
- Three pre-existing tests call `sum(expr.search_string(...))` (lines 4962,
  5236, 5262; blame predates #642), so an `__radd__` that returns
  `NotImplemented` unconditionally fails them; #642 kept the `0 + pr` case.
- For the `Dict` fix the maintainer rejected the PR's first version: "There
  is a bug in `__setstate__()`, fails to preserve value of
  `_is_dict_context`. Add unit tests to verify fixed behavior, and
  persistence across pickle/unpickle" (ptmcg on #635); `testParseResultsPickle`
  1 to 3 are pre-existing.
- 9b728b8 (2026-06-26) is a maintainer revert of unrelated formatting that a
  contributor's `as_datetime` fix dragged in, showing the review bar.

Build and test: pure Python, no runtime deps. `tests/test_unit.py`: 1952
passed, 6 skipped, 2026 subtests in 23 s. `tests/test_diagram.py` imports
`railroad` (install `railroad-diagrams` or deselect). Note the follow-ups are
direct pushes by the maintainer, not PRs.

Contamination: medium. PR bodies include repros; #645 and #648 were written
with Claude. Suggested base: 2026-06-23 (before 16f3486) with #642, #643,
#645 (+6f2fbcb), #648 and a6ae98b as gold.

### C5. croniter: `expand_from_start_time` phase and range expansion

Repo: https://github.com/pallets-eco/croniter (MIT; the project moved under
pallets-eco in 2026). File: `src/croniter/croniter.py` (1586 lines). Tests:
`src/croniter/tests/test_croniter.py` (3617 lines), `test_croniter_hash.py`,
`test_croniter_range.py`.

Fixes (merged, source lines, tests):

- #235 (2026-07-02, f329964) month low bound used `dt.month % step` for a
  1-based field; +1 -1; `test_expand_from_start_time_month` extended.
- #239 (2026-07-09, 44e3090) Sunday must wrap to 0 before the step modulo
  (`(dt.isoweekday() % 7) % step`); +1 -1; day-of-week test extended.
- #246 (2026-08-01, f64665e) `{start}/{step}` with `start == max` expanded to
  the whole field because it collided with the explicit equal-range rule;
  +32 -2; `test_step_from_field_max`, which also asserts `5-5 * * * *` still
  expands to `["*"]`.
- #253 (2026-08-01, e15047e) `HashExpander` offsets clamped to the declared
  range; +26 -14; `test_croniter_hash.py`.
- #254 (2026-08-01, 269121b) seconds and years fields raised `ValueError` in
  `_get_low_from_current_date_number`; +9 -2; tests added.
- #256 (2026-08-01, f1f3b2e) phase taken from the start time's own tzinfo
  instead of UTC; +10 -4; tests added, including one pinning that an
  explicitly UTC start agrees with the naive one.
- Related: #243 (2026-07-28) unsatisfiable day-of-month OR day-of-week;
  #232 (2026-07-02) reject zero step in equal and reversed ranges; #255
  (still open on 2026-10-24) proposes making `expand_from_start_time` supply
  only the phase of a cycle.

Interaction evidence: #253 body: "Found while reviewing #246, which fixes
the same 'equal range means the whole field' collision on the non-hashed
side." #256 body: "Found by an adversarial pass over #246/#253/#254/#255,
the one probe the whole stack still tripped." #235, #239, #254 and #256 all
edit `_get_low_from_current_date_number`.

Floor 8 evidence: the obvious fix for #246 (treat `low == high` as a single
value) destroys the documented rule that `Jan-Jan`/`Sun-Sun` mean the whole
cycle; #246 added assertions for `1-1`/`5-5` and the 6.2.5 changelog documents
the rule ("a range with two equal bounds (`Jan-Jan`, `Sun-Sun`) means the whole
cycle rather than the value it names"). #253's body: "Divisors that fit their
range keep their existing hashes ... so no schedule that was already correct
moves", and the hash tests pin existing hashes. #256: "Naive start times are
untouched ... Verified over a 45,312-expansion sweep". The obvious fix for
#239 (use `dt.weekday()` with a different offset) moves Monday to Saturday,
which the existing day-of-week tests with Wednesday and Thursday starts catch.
Whether a pre-#246 test pins the equal-range rule is not verified (see (d)).
URLs: https://github.com/pallets-eco/croniter/pull/246,
https://github.com/pallets-eco/croniter/pull/253,
https://github.com/pallets-eco/croniter/pull/256.

Build and test: pure Python; deps python-dateutil, pytz (and tzdata on
slim images); pytest; roughly 230 tests in seconds (not timed locally).

Contamination: medium; PR bodies include repros and tables. #246 came from an
automated "Coretexa" pipeline. Suggested base: 2026-07-01 (before #235), gold
#235, #239, #246, #254, #256 (+ #253 if the hash module is wanted).

### C6. pint: `wraps()` and `check()` in `registry_helpers.py`

Repo: https://github.com/hgrecco/pint (BSD-3-Clause). File:
`pint/registry_helpers.py` (509 lines). Tests: `pint/testsuite/test_unit.py`
(`test_wraps`, `test_wrap_referencing`, `test_check*`), `test_issues.py`,
`test_wraps_varargs.py` (296 lines, added by #2422).

Fixes (merged, source lines, tests):

- #2325 (2026-07-26, 5aff6a4) pass the registry to `_parse_wrap_args` so
  `"dimensionless"` input specs resolve; +1 -1; `test_issue2255`.
- #2336 (2026-07-26, a86c548) skip `**kwargs` when packing and unpacking,
  keep the trailing-`None` workaround working; +15 -6; tests in `test_unit.py`.
- #2422 (2026-10-11, c0d1394) bind variadic calls by signature, convert every
  `*args` element, empty variadic reference is dimensionless; +96 -37;
  `test_wraps_varargs.py` (43 tests, "29 failed, 13 passed" on the base).
- #2426 (2026-10-11, 6d127d9) `check()` binds variadic and keyword-only
  signatures before checking; +50 -13; six new tests in `test_unit.py`.
- #2460 (2026-10-12, b599024) typing only; +26 -6; pyright-checked module.

Interaction evidence: #2422 body: "Related prior work: #2336 handles
`**kwargs`, whereas #2326 was closed under the assumption that #2336 might
also address `*args`. The reporter subsequently confirmed that #2254 still
reproduced." #2426 applies the same binding approach to `check()` and both
require a `None` placeholder for `**kwargs`.

Floor 8 evidence (weaker): pre-existing `test_wraps` and `test_check` pin
strict-mode `TypeError`s, the `(None,)` return spec forms and the trailing
`None` for `**kwargs` (`test_unit.py` lines 544 to 631, 704 to 722 at the
#2336 base), so a fix that changes slot counting breaks them. The codspeed bot
flagged 20 to 25 percent slowdowns on `test_wrapper_nonstrict` for #2325 and
#2336 (not test failures). #2426: "Positional signatures retain a direct
argument path ... Fixed-argument microbenchmarks showed no slowdown."

Build and test: pure Python; runtime deps platformdirs, typing_extensions,
flexcache, flexparser; numpy needed for most of the suite; measured 2786
passed, 26 skipped, 10 xfailed in 41 s when `pint/testsuite/benchmarks` is
excluded (it needs pytest-benchmark). The wraps-related files alone: 432
passed in 10 s.

Contamination: medium; #2422 and #2460 written with Codex, #2444 with Claude
Code. Suggested base: 2026-07-25 (before #2325), gold #2325, #2336, #2422,
#2426.

## (b) Predicted naive fix and the guard it should break

C1 black. Symptoms: crashes ("produced invalid code", "list index out of
range", "produced different code on the second pass") or reformatted skipped
text for `# fmt: skip` on brackets, headers and one-liners. The competent
first fix widens the skipped region to the whole statement or logical line
(or bails out of `# fmt: skip` handling for the problem shape) so the crash
disappears. That reformats or freezes text the directive did not cover and
fails the pre-existing `fmtskip*.py` and `fmtonoff*.py` case files through
`assert_stable`/`assert_equivalent`; the most likely first casualty is
`fmtskip_header_bracket_5402.py` or `fmtskip_paired_header.py`, because a
blanket "never split after a converted standalone comment" (#5538's area)
breaks headers whose same line ends in a trailing `# fmt: skip`, which must
still split (#5538: "If there is any `#` on that line ... the split stays as
before"). Likewise, dropping the appended newline in `stringify_node` (#5597)
breaks #4872's trailing-comment case. Upstream's own partial fixes (#5405,
#5401) were each followed by a correction, which is the floor-8 pattern.

C2 tomlkit. Symptoms: invalid TOML accepted (or only failing on access) for
out-of-order redefinitions; dotted-key replacements that capture following
siblings; duplicate `[x]` headers. The naive parser fix rejects any
out-of-order fragment whose key already exists as a concrete table or dotted
key. It breaks `test_valid_out_of_order_independent_tables`,
`test_toml_document_super_table_with_different_sub_sections`,
`test_values_can_still_be_set_for_out_of_order_tables` and the toml-test
valid files (and, if present at base, #565's dotted-super-table case). The
naive rendering fix always moves a table that replaces a dotted key to the
end of the document or always promotes dotted keys to headers; it breaks
`test_replace_dotted_key_with_table`'s exact rendering,
`test_toml_document_with_dotted_keys` round-trip and `test_add_dotted_key`,
and the two tests #567 names that pin emptied super tables vanishing.

C3 sqlglot. Symptoms: optimized queries return different rows than the input
(DuckDB disagrees) for correlated `EXISTS`, non-EQ correlation keys and
subqueries in `JOIN ON`. The naive fix refuses to decorrelate whenever a
subquery sits in a `JOIN ON`, has a non-EQ predicate, or projects any
aggregate. Each refusal fails existing fixtures that expect the rewrite: the
`<> ANY` and `ARRAY_AGG` fixtures at the top of `unnest_subqueries.sql`, the
inner-join `ON` fixtures in `optimizer.sql` from #8638, and the
`EXISTS (SELECT COUNT(*) ...)` executor tests that expect `TRUE` folding. The
opposite naive fix for the aggregate case (fold whenever an `AggFunc` is
present) fails the `HAVING`/`QUALIFY`/windowed fixtures. The `execute: true`
fixtures also catch rewrites that are syntactically plausible but wrong.

C4 pyparsing. Symptoms: `copy()` mutation corrupts the original's names,
`deepcopy()` names alias the original, `1 + pr` recurses, `pr + 1` raises
`AttributeError`, empty `Dict` serialises as `[]`. The naive fix makes
`copy()` deep (or copies `_tokdict` values eagerly) and makes `__radd__`
return `NotImplemented` for everything. It fails `testParseResultsCopy`
(`r2[1] is result[1]`) and the three `sum(expr.search_string(...))` tests. A
naive `Dict` flag that is not carried through `__getstate__`/`__setstate__`
fails the pickle tests, and a flag that marks every empty result fails the
tests expecting `[]` for empty `ZeroOrMore(...)("name")` results.

C5 croniter. Symptoms: `expand_from_start_time` raises or drops the start
value for some months, Sundays, seconds/years fields and aware timezones;
`59/15` fires four times an hour. The naive fix treats any `low == high`
range as a single value, which fails the explicit equal-range assertions
(`5-5 * * * *` expands to `["*"]`, per #246 and the changelog). The naive
day-of-week fix shifts the Monday to Saturday phase and fails the existing
Wednesday/Thursday start tests. The naive timezone fix (convert the start to
UTC consistently) changes naive-start results, which the existing
`expand_from_start_time` tests pin.

C6 pint. Symptoms: `wraps` raises `AssertionError` for `"dimensionless"`,
mis-converts `*args`, rejects `**kwargs`; `check()` validates the wrong
argument for keyword-only parameters. The naive fix counts positional values
against the unit list and pads or truncates, which changes slot semantics and
fails the pre-existing `test_wraps` strict-mode `TypeError` cases and the
trailing `None` placeholder behaviour; binding every call through
`inspect.signature` also fails the positional-only default path #2426 kept.
This candidate's floor-8 case is the least certain of the six.

## (c) Survey list

One line per repository looked at. "Kept" means it appears in (a); "reserve"
means a plausible cluster that was not fully verified this wave.

- psf/black: kept (C1). 224 merged PRs since 2026-06-01; `# fmt: skip` cluster in one file.
- python-poetry/tomlkit: kept (C2). 97 merged PRs; container.py cluster.
- tobymao/sqlglot: kept (C3). 617 merged PRs; unnest_subqueries cluster. Also viable: `merge_subqueries.py` (#8379, #8425, #8427, #8578, #8623, #8646, 2026-09-18 to 2026-10-16) and `simplify.py` (22 commits since August).
- pyparsing/pyparsing: kept (C4). 14 merged PRs; ParseResults cluster. Also a small `as_datetime` cluster (#639, #641, #647 plus revert 9b728b8).
- pallets-eco/croniter: kept (C5). 15 merged PRs; expand_from_start_time cluster.
- hgrecco/pint: kept (C6). 124 merged PRs; registry_helpers cluster. Also viable but numpy-heavy: `facets/numpy/numpy_func.py` (18 fixes 2026-07 to 2026-10) and offset/log units (#2318, #2484, #2486, #2458).
- more-itertools/more-itertools: reserve. `numeric_range` has three causes in one class (#1216 2026-07-17, #1248 2026-09-02, #1276 2026-09-17) and `bucket` has an explicit floor-8 statement (#1285: "would break the existing `test_list`") plus a maintainer rewrite (#1288, Raymond Hettinger: "too aggressive") and #1309; but bucket has only two root causes and the two classes are unrelated, so it fails the "one class" bar unless the module counts. Tests: 765 passed in 13 s.
- lepture/mistune: reserve. `renderers/markdown.py` (310 lines) received about 15 round-trip fixes 2026-06-14 to 2026-10-19 (#449, #450, #459, #460, #462, #469, #481, #484, #485, #486, #487, #496, #500), each with tests and a built-in round-trip oracle; #450 and #484 both edit `codespan`. BSD-3, pure Python. Not read deeply enough to rank.
- Python-Markdown/markdown: reserve. Backtick-span cluster #1618 (2026-08-08), #1625 (2026-08-30), #1669 (2026-10-21) in `inlinepatterns.py`; emphasis rewrite #1632 then partial revert #1659 ("Revert enforcement of CommonMark punctuation rules") is a floor-8 shape but a 1200-line rewrite. BSD-3.
- cpburnz/python-pathspec: reserve. MIT (checked via API). Gitignore cluster #132, #138, #139, #144, #164, #165 (2026-09 to 2026-10) with explicit follow-up chains ("Fixes part B of #137", "the residual `d/**/` case discussed in merged #138"); each fix is mirrored across three backends (simple, re2, hyperscan), which widens the footprint.
- facelessuser/wcmatch: reserve. MIT. Glob cluster #242, #243, #244, #245, #246, #247, #249 (2026-10-15 to 2026-10-20); #244 explicitly separates itself from #243; #247 is a regression from an 8.1.1 fix. Not read deeply.
- python-humanize/humanize: reserve. Rounding-carry cluster #328, #329, #346, #354, #359 (2026-06-30 to 2026-09-16) in `number.py`/`filesize.py`; #359 amends #328. Small codebase, likely under two expert hours.
- jab/bidict: rejected. Strong rollback cluster (#391, #392, #401, #403, #413, #414) but licence is MPL-2.0.
- HypothesisWorks/hypothesis: rejected. MPL-2.0 and a Rust extension since 2026-07-01 (#4776 "Initial rust release").
- python-pendulum/pendulum: rejected. Rust extension; fixes scattered (locale, pickling).
- pydantic/pydantic: rejected. Correctness lives mostly in pydantic-core (Rust); pure-Python fixes are spread across schema generation.
- dateutil/dateutil: rejected. 3.0.0 churn (zoneinfo backport #1130, drop Python < 3.11); no concentrated correctness cluster.
- lark-parser/lark: rejected. 16 PRs, fixes spread across lexer, Earley and Reconstructor.
- python-babel/babel: rejected. Plural-rule data fixes (#1303 then #1309 "correctly", #1318) are CLDR data driven.
- marshmallow-code/marshmallow: rejected. 28 PRs, fixes scattered (Pluck, URL validator, TimeDelta).
- python-attrs/attrs: rejected. 25 PRs, no cluster.
- python-attrs/cattrs: rejected. Two disambiguator fixes (#765, #778) and three codegen quoting fixes (#769, #771, #777) that are the same bug class repeated; thin floor-8 story.
- Textualize/rich: rejected. 1 merged PR.
- arrow-py/arrow, pallets/jinja, pallets/markupsafe: rejected. 0, 0 and 9 merged PRs.
- pallets/click: rejected. Mostly deprecations; pager cluster (#3764, #3767, #3776, #3777) spawns subprocesses.
- pallets/werkzeug: rejected. 4.0 deprecation churn; routing converter strictness cluster (#3235, #3238, #3243, #3254, #3255) is maintainer hardening, not reported bugs.
- python-jsonschema/jsonschema: rejected. 4 merged PRs.
- pygments/pygments: rejected. 67 PRs but one lexer each; no shared subsystem.
- andialbrecht/sqlparse, yaml/pyyaml: rejected. 6 and 2 merged PRs.
- executablebooks/markdown-it-py: rejected. 16 PRs, no cluster.
- scrapinghub/dateparser: rejected. 107 PRs, mostly new parsing features and language data; heavy data dependencies.
- collective/icalendar: rejected. 218 PRs, mostly features (xCal, jCal, typing).
- PyCQA/isort: reserve. Trailing-comment cluster (#2647, #2651, #2675, #2686, #2733) and indented-block cluster (#2730, #2672, #2738) 2026-09 to 2026-10; mypyc wheels now exist but source install is pure Python; not investigated.
- mahmoud/boltons: rejected. 44 PRs across unrelated utility modules.
- jsonpickle/jsonpickle: rejected. Fixes centre on numpy/pandas extensions.
- pypa/packaging: rejected. Scattered; small pylock `select()` cluster (#1349, #1362, #1376).
- beartype/beartype: rejected. Forward-reference cluster (#691, #709, #716) but the suite is slow and the codebase is unusually hard to read.
- agronholm/typeguard, pytoolz/toolz, SethMMorton/natsort, tkem/cachetools, kjd/idna, r1chardj0n3s/parse, Delgan/loguru: rejected. Under 20 PRs each, no cluster.
- Instagram/LibCST: rejected. Rust native parser.
- pytest-dev/pytest, pylint-dev/pylint, pylint-dev/astroid, python/mypy, sphinx-doc/sphinx, sqlalchemy/sqlalchemy: rejected. Too large for the "one subsystem, under 20 minute suite" bar without a dedicated pass; not surveyed in depth (rate limit hit).
- nedbat/coveragepy, astanin/python-tabulate, chaimleib/intervaltree, AlexandreDecan/portion, grantjenks/python-sortedcontainers, seperman/deepdiff, mahmoud/glom, davidhalter/parso, erikrose/parsimonious, pytransitions/transitions, tartley/colorama, python-jsonschema/referencing, jmespath/jmespath.py, mkdocs/mkdocs, tiangolo/sqlmodel, ijl/orjson: rejected. 0 to 4 merged PRs since 2026-06-01 (or no longer at that path).
- pyca/pyopenssl: rejected. Security-adjacent.
- pandas-dev/pandas, scikit-learn/scikit-learn, pydata/xarray, shapely/shapely: not surveyed (rate limit, and compiled or very large suites).

## (d) Gaps and uncertainties

- Floor 8 is argued from the public record only. For each candidate the
  authoring step must apply the predicted naive fix at the chosen base and
  confirm which existing test fails; the guard names above are the most
  likely, not verified, casualties.
- C1 black: `comments.py` changed five times in the last three days of the
  window (2026-10-22 to 2026-10-24); a base must be pinned to a specific main
  commit and the gold subset chosen so the fixes apply cleanly. The fmt: off
  fixes (#5549, #5585) touch the same functions and would need to be in or
  out deliberately. blackd and the `[d]` extra can be skipped.
- C2 tomlkit: the `tests/toml-test` submodule pin (08ed8697) must be vendored
  into the image; its 1.0.0 and 1.1.0 file lists are both present. The
  cluster spans four months (#516 in June, #565 in October); at a June base
  the #565 regression test is the natural hidden guard.
- C3 sqlglot: #8742 corrected an existing `optimizer.sql` fixture that pinned
  wrong output; at a pre-#8742 base that fixture contradicts the gold and must
  be removed or corrected in the task's test set. #8638's tests live only in
  the `sqlglot-integration-tests` submodule (needs live engines), so its
  regression coverage inside the repo is indirect. Several fixes are marked
  `!` (behaviour change) and a few fixtures carry `execute_min_python`.
- C4 pyparsing: all tests live in one 12k-line file, so hidden and visible
  tests must be split by test method, not by file. Two gold pieces (6f2fbcb,
  a6ae98b) are direct maintainer commits without PR threads.
- C5 croniter: #255 (open) overlaps the same function and may land before the
  task is built; whether a pre-#246 test pins the equal-range rule is not
  verified (the 6.2.5 changelog says the rule "was undocumented"). Expert
  hours may fall under two for a fast engineer.
- C6 pint: floor-8 evidence is the weakest; the review friction on #2325 and
  #2336 was codspeed performance, not failing tests. numpy is needed for the
  bulk of the suite, and `pint/testsuite/benchmarks` must be excluded or
  pytest-benchmark installed.
- Contamination: every cluster here has detailed public root-cause write-ups
  (often AI-authored) and many fixes were merged in October 2026, so a model
  with a late-2026 cutoff could have seen them. Task text should give
  symptoms and failing behaviour only, never PR numbers or test names.
- Rate limiting on the GitHub search API cut the first survey pass short for
  about 25 repositories (listed as rejected or not surveyed above); isort,
  mistune, pathspec and wcmatch deserve a second pass if more Python
  candidates are needed.
- Test timings were measured on an M-series Mac, not in the arm64/x86-64
  Docker images; expect 1.5 to 3 times slower in CI containers.
