# F5 wave 1: sourced candidates under floor 8 (pre-registration, 2026-10-24)

Status: **sourced from the public record only.** Nothing here is built, run,
probed or gated. This document pre-registers, before any task is authored,
which upstream fix clusters wave 1 will try for the five open F5 slots, why
each is expected to meet floor 8 (PLAN-v3.md section 2, calibrated in
FLOOR8-CALIBRATION.md), what the predicted naive fix is and which existing
test it should break, and what the authoring step must verify. Gate runs
need the owner's approval per run, as for every Frontier v5 run.

## Method

Five searches ran in parallel on 2026-10-24, one per open language, each over
the merged pull requests of 44 to 70 actively maintained repositories since
2026-06-01 (about 300 repositories in all), using the GitHub API, PR bodies,
review and issue threads, diffs and shallow clones of the leaders. A
candidate had to show, from the record: at least three root causes fixed in
one subsystem since June 2026, each with its own regression test (floor 1);
source coupling or a test that needs two fixes (floor 3); and floor 8
evidence of one of four kinds: a first fix merged and then reverted or
amended because it regressed other behaviour, fix PRs adding tests that
already pass before the fix, a maintainer rejecting a local fix in review,
or test names that negate the obvious rule. Every repository surveyed is
listed with a one-line verdict in `sourcing/wave1/<language>.md`, so the
sweep is not repeated. Already used, rejected or swept projects were
excluded (GEOS, fmt, yyjson, tree-sitter, mruby, Commons Lang, NetworkX,
SymPy, petgraph, iOverlay, comrak, luxon, typescript-eslint, marked,
csv-stringify, cron-parser), as was memory-safety work.

Floor 8 is judged here from the record. The calibration showed the record
can be read correctly (tree-sitter's six frontier shortcuts all turned up
where its design predicted), but only the real verifier run on a stored
naive patch counts.

## The slate

| Slot | Primary | Reserve | Second reserve |
| --- | --- | --- | --- |
| F5-rust | pulldown-cmark, HTML block and inline HTML scanning | sqlparser-rs, unary-operator display round trip | rust_decimal, scale and rounding |
| F5-python | black, `# fmt: skip` handling | tomlkit, out-of-order and super tables | sqlglot, subquery decorrelation |
| F5-java | javaparser, symbol-solver member resolution | jsoup, tokeniser and template tree building | jackson-core, tokenizer backends |
| F5-cpp | Boost.URL, path normalization and compare | ada-url, URLPattern canonicalization | fkYAML, block scalar parsing |
| F5-javascript | prettier, comment attachment around dropped parentheses | babel, block-scoping for-loop head lowering | zod, optionality ladder |

Build order, cheapest validation first: black, pulldown-cmark, prettier,
then Java (jsoup if build time rules out javaparser), then Boost.URL.

## F5-rust: pulldown-cmark (MIT)

Subsystem: the CommonMark block first pass and the HTML scanners,
`pulldown-cmark/src/firstpass.rs`, `scanners.rs`, `parse.rs`. Cluster A,
2026-10-08 to 2026-10-19: #1164 (def3889), #1165 (953e44a), #1163 (a3df587,
two causes), #1169 (ad1622b, three causes), #1180 (044f471), plus #1116.
A second cluster on tab and container prefixes (#1150, #1151, #1159, #1172,
#1173, #1170, #1183) is recorded but not used, because #1159 changed two
existing test expectations.

- Interaction: #1169 reuses the newline handler #1165 introduced; #1163,
  #1167, #1169 and #1170 were rebased on #1165.
- Floor 8 evidence: the maintainer closed a local fix in favour of a
  structural one six times (#1166 for #1172, "The bug is actually in
  scan_containers"; #1177 for #1180, "needlessly complex"; #1134 for #1183;
  #1124 for #1135; #1113 for #1116; #1145 for #1148); a review comment on
  #1169 reads "I deleted them, and all of the tests still pass"; 652
  CommonMark spec tests pin adjacent behaviour.
- Predicted naive fixes and the guards they should break: case-folding the
  end tag where it is compared (#1163), scanning forward from every line to
  decide an HTML block's end (#1164), stripping the container prefix from
  every continuation line (#1165), and allowing a newline inside an inline
  tag (#1169); each is a per-symptom change in the scanner that the spec
  suite's lazy-continuation and HTML-block examples reject. Full list in
  `sourcing/wave1/rust.md`.
- Size and time: gold 150 to 250 source lines; 4 to 8 expert hours.
- Feasibility: `Cargo.lock` committed, MSRV 1.70, pure Rust;
  `cargo test -p pulldown-cmark`, excluding the fuzz workspace member, which
  pulls in mozjs and bindgen.
- Reserve: sqlparser-rs (Apache-2.0), eleven PRs 2026-09-21 to 2026-10-19 on
  `Expr::UnaryOp` display, with a fix-of-a-fix chain in one match arm
  (#2532 probe, #2541 AST walk, #2577 early-stop probe with a timeout test);
  3,545 round-trip assertions pin canonical output. Caveat: #2531 and #2533
  rewrote existing unary-operator tests, so hidden tests must be the
  post-fix versions. Second reserve: rust_decimal (MIT), five scale and
  rounding fixes 2026-07-17 to 2026-10-19 whose PR text names the existing
  test a shortcut would regress ("None would regress that test"); gold about
  50 lines, likely below size.

## F5-python: black (MIT)

Subsystem: `# fmt: skip` handling in `src/black/comments.py` and two spots
in `linegen.py`. Ten fixes 2026-09-22 to 2026-10-24: #5401, #5405, #5429,
#5464, #5522, #5538, #5584, #5586, #5588, #5597, each with a
`tests/data/cases/fmtskip_*.py` case file.

- Interaction: the fixes edit one comment-handling path in sequence; a
  five-fix subset is the fallback if the full ten do not compose cleanly.
- Floor 8 evidence: three of the merged fixes are regressions of earlier
  fixes (#5405 from #5144, #5429 from #4552, #5549 from #5401), and every
  case file is checked for idempotence and AST equivalence, so a shortcut
  that produces plausible output fails the second formatting pass or the
  AST check.
- Predicted naive fix and guard: special-case the reported constructs where
  the trailing `# fmt: skip` is detected, which breaks idempotence on the
  existing `fmtskip_*` cases and the AST-equivalence check.
- Size and time: gold about +155/-28 for ten fixes, about +90/-20 for the
  five-fix subset; 6 to 12 expert hours.
- Feasibility: pure Python, 628 tests in 14 seconds locally. The base must be
  pinned to a specific day between fixes; the PR bodies carry detailed
  root-cause write-ups, several from October 2026, so the instruction must
  hide identifiers.
- Reserve: tomlkit (MIT), ten fixes 2026-06-15 to 2026-10-12 in
  `tomlkit/container.py`; #565 repairs an over-rejection that #516 and
  #530 shipped (which is exactly the naive fix), and #567 names the two
  existing tests the obvious rendering fix breaks; the vendored toml-test
  suite guards the parser; gold about +158/-11; 5 to 10 hours; 1,203 tests
  in under a second. Second reserve: sqlglot's `unnest_subqueries.py`
  decorrelation, nine fixes 2026-08-21 to 2026-10-23 with a corrected
  fixture (#8742); gold about +296/-73; needs duckdb and pandas wheels.

## F5-java: javaparser (Apache-2.0 or LGPL, dual)

Subsystem: the symbol solver's member resolution,
`MemberResolutionLogic.java` (created by #5139) and the adapters around it.
Fixes #5139 and #5141 (2026-09-18), #5145 and #5147 (2026-09-24), #5154 and
#5157 (2026-10-10), each with its own `IssueNNNNTest` and no existing test
modified. #5159 is excluded (it changes two existing least-upper-bound
expectations).

- Interaction: the class's whole history is this cluster (#5139 creates it,
  #5141 moves code into it, #5154 extends it); #5147 and #5154 edit the
  same `isApplicable` region.
- Floor 8 evidence: the PR bodies name the existing tests the members-only
  fix had to keep green (#5139: Issue3200Test, Issue4047Test,
  Issue4358Test, CompilationUnitContextResolutionTest, enum `values()` and
  `valueOf`; #5141: JavaParserTypeDeclarationAdapterTest.issue3550; #5147
  keeps #4890 working, guard MethodsResolutionLogicTest
  .testVariadicBoxedToPrimitive, verified at the pre-fix base); in #5154 the
  maintainer rejected the reporter's `isMoreSpecific` fix as "silently wrong
  for the second one".
- Predicted naive fix and guard: resolve the reported member by name in the
  declaring type only, or special-case the failing overload in
  `isMoreSpecific`; the named Issue tests and the variadic boxing test are
  the guards.
- Size and time: gold about +395/-141 for the four-fix subset, about
  +900/-410 for all six; 8 to 16 or 12 to 24 expert hours.
- Feasibility: Maven multi-module; the CI build-and-test job takes about 14
  minutes, so the verifier must restrict to
  `-pl javaparser-symbol-solver-testing -am`. The fixes implement the Java
  Language Specification, so the difficulty must come from the guards, not
  from knowing the right answer.
- Reserve: jsoup (MIT), a seven-fix tokeniser and template subset of
  #2581, #2596, #2615, f79f21d2, 81718491, f062a8ab, #2635 and #2636
  (2026-08-22 to 2026-09-29) that modifies no existing test; floor 8 from
  #2636 keeping the XML parser's EOF recovery "as we had specific coverage
  for it" (guard XmlTreeBuilderTest.testDoesHandleEOFInTag, verified at
  base) and from templateFragmentContextCannotBeClosedByInputAtEof, which
  forbids the naive pop-all-templates loop #2615 needed; gold about
  +99/-91; 4 to 8 hours; 1,959 tests in about 2 minutes. Second reserve:
  jackson-core tokenizer backends (#1743 to #1796, October 2026), a heavy
  refactor with a SNAPSHOT parent POM to pre-cache.

## F5-cpp: Boost.URL (BSL-1.0)

Subsystem: path normalization and comparison, `src/detail/normalize.cpp`
and `impl/url_base.hpp`. PR #1009 (merged 2026-10-16) carries nine
one-commit fixes: the compare hang on cancelled relative paths (#1003), a
trailing slash after dot segments, the "./" shield, hash consistency with
compare, segment count after the shield, and four decoded-path-size
bookkeeping bugs (#1008). As with nx-digraph, one merged change fixing
several causes means the gold is split per cause for the controls.

- Interaction: the maintainer's commit message states one fix cannot ship
  without another (below).
- Floor 8 evidence: "A version that decoded it, as the old code did, would
  make the existing test compare("../a/b", "..%2Fa/b") return 0 instead of
  +1, so the encoded slash fix has to be part of this commit"; pre-existing
  tests assert hash equality for every pair that compares equal, so a
  compare-only fix fails them.
- Predicted naive fix and guard: fix compare's termination and decoding
  locally without touching hash or the encoded-slash handling; the existing
  compare and hash-consistency tests are the guards.
- Size and time: upstream rewrote normalize.cpp (+707/-347); a minimal fix
  is estimated at 150 to 250 lines; 6 to 10 expert hours.
- Feasibility: needs the Boost 1.92.0 superproject vendored for the build;
  build time is unmeasured and is this candidate's main risk. The exact
  1.92.0 release date was not confirmed (cadence implies August 2026,
  before #1009).
- Reserve: ada-url (Apache-2.0 or MIT), eleven URLPattern canonicalization
  PRs 2026-07-03 to 2026-10-19 in `url_pattern_helpers.cpp` and
  neighbours, each with a WPT-style test; #1171 ("the two strips
  cancelled"), #1190 and #1215 coupled, #1191 and #1329 fixing the same
  predicate twice; gold about 90 lines; 4 to 8 hours; simdjson and gtest
  must be vendored. Second reserve: fkYAML (MIT) block scalars (#589,
  #610, #618, #620, #632, #633), where #616 fixes a regression #609
  introduced; doctest and yaml-test-suite to vendor.

## F5-javascript: prettier (MIT)

Subsystem: comment attachment around dropped parentheses,
`src/language-js/comments/handle-comments.js` and `attach.js`. Six core
fixes (#19262, #19263, #19276, #19893, #19894, #19930; June to August
2026) plus the adjacent #19345 and #19652.

- Interaction: one handler extended in sequence; #19893 changes `attach.js`
  to expose ancestors and #19930 edits the exact condition #19893 added.
- Floor 8 evidence: #19273, the obvious generalised fix, was merged and
  reverted the same day by #19274 for breaking `prettier-ignore`d nodes (a
  SyntaxError on the second pass); #19893 says "Scope is deliberately
  narrow, since #19273 was reverted once already"; #19263 names the
  contexts it deliberately leaves unchanged.
- Predicted naive fix and guard: generalise the comment re-attachment to
  every parenthesised node, as #19273 did; the `prettier-ignore` format
  tests and the second-pass idempotence check are the guards.
- Size and time: gold about +135/-50; 6 to 12 expert hours.
- Feasibility: yarn 4, jest run from source; the verifier should restrict
  to the JavaScript and TypeScript format test directories. Some fixes
  date from June 2026, at the edge of the cutoff rule.
- Reserve: babel (MIT), `@babel/plugin-transform-block-scoping` for-loop
  head and closure lowering, six fixes 2026-07-15 to 2026-10-20 (#18088,
  #18193, #18194, #18195, #18320, #18331); the maintainer rejected the
  naive "snapshot at closure creation" semantics in review on #18195
  ("later mutation in the same head should leak into it"), the fixture
  `for-head-closure-mutated-in-head` pins it, and #18331 resolved a
  fixture #18195 left knowingly wrong; #18331 uses the `inHead` flag
  #18195 introduced; gold about +125/-26; 5 to 9 hours; yarn 4 with a
  heavy `make build`. Second reserve: zod's v4 optionality ladder, whose
  exactOptional parse prototype was reverted two days after merge.

## Cross-cutting notes

- Contamination. Every cluster postdates June 2026, but many PRs carry
  complete, AI-authored root-cause write-ups. Instructions must be
  symptoms-only with no issue or PR identifiers, as the task contract
  already requires, and DESIGN.md records the merge dates.
- Base selection. Several clusters contain a fix that changed an existing
  test expectation (pulldown-cmark #1159, sqlparser-rs #2531 and #2533,
  jsoup #2616 and #2618, javaparser #5159, magic-string #349, picomatch
  #199). Wave 1 uses the subsets that modify no existing test, and hidden
  tests are always the post-fix versions.
- Specification-implementing projects (javaparser, jsoup, Boost.URL,
  ada-url) have a public right answer. Their difficulty has to come from
  the implementation guards; the naive-fix control is how that is checked.
- Expert time is recorded as an estimate per candidate here and as a field
  in each DESIGN.md once authored, as PLAN-v3 recommends.

## What authoring must verify, per candidate, before any gate run

1. Base construction: a pinned single-commit workspace with the fix history
   stripped, held-out tests deleted (never reverted), chosen so that no
   existing test expectation changes.
2. Floors 1 to 7 with `run_controls.py`: per-cause parts, single-fix and
   leave-one-out variants, gold-from-parts reward 1.
3. Floor 8 with a stored `naive:` control written from the prediction
   above: it must build, clear at least one held-out check and regress at
   least one guard. If it does not, record it and try the reserve; do not
   search for a different naive fix until one trips.
4. The three verifier layers and the cheating probes (PHASE1.md, "Verifier
   audit 2026-10-08"), base 0 and gold 1 three times, through the real
   Harbor oracle and nop paths.
5. Build and test time measured in the arm64 image, against the 20-minute
   verifier budget and the 3-hour agent budget.
6. An expert-time estimate and the predicted miss in DESIGN.md.

## Cost and yield

Five primaries and ten reserves. At the yield seen so far (one admit in six
candidates) this wave should be expected to produce one or two admits and
to tell us, within the wave, whether floor 8 generalises beyond
tree-sitter. Authoring cost is roughly a session per candidate; gate cost
is 3 to 10 runs per candidate, Astra first, each on the owner's approval.
The halt rule of PLAN.md section 7 applies: three consecutive rejects in
the family stop the wave for a post-mortem.

## Owner decisions

(k) Approve this slate, or swap a primary for its reserve. The swaps most
worth considering: jsoup for javaparser if the Maven build cannot be made
fast enough, and ada-url for Boost.URL if vendoring Boost 1.92.0 proves
heavy.

(l) Approve the build order (black, pulldown-cmark, prettier, Java, C++),
or reorder.
