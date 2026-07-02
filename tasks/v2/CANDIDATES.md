# v2 candidate pipeline

Status values: `candidate` (sourced, not built), `building`, `admitted` (measured in
70-90 band), `rejected-easy` (frontier >90%), `rejected-bad` (flaky/ambiguous/craters).
Difficulty column is a provisional read from the PR; the admission rule (CHARTER.md)
decides via a measured run.

## Batch 1 (swept 2026-06-30: click, jinja, werkzeug, packaging, more-itertools, attrs,
## jsonschema, arrow, marshmallow, slugify, charset_normalizer; post-cutoff window >= 2026-02-01)

| # | PR | merged | bug (paraphrased) | provisional difficulty | build risk | status |
|---|----|--------|-------------------|------------------------|-----------|--------|
| 1 | pallets/werkzeug#3163 | 2026-04-23 | A Range request whose If-Range validator is a WEAK ETag must be ignored (serve full 200), because If-Range requires a strong validator (RFC 7232). Code honored the weak validator and served a 206. | HIGH: naive fix compares ETags and misses that If-Range is strong-only, while If-None-Match accepts weak. Multi-file (range parse + request). | LOW | candidate |
| 2 | pallets/click#3582 | 2026-06-14 | Resolving a command's package/version name breaks when the import module name differs from the installed distribution name (module `foo`, distribution `foo-cli`). | MED-HIGH: needs importlib.metadata module-to-distribution mapping; naive use of the module name fails. | MED (tests mock distributions; slice with care) | candidate |
| 3 | pypa/packaging#1299 | 2026-06-27 | An invalid version specifier combining a post-release with a prefix wildcard (e.g. a `.post1.*` form) should raise a clear error; the parser mishandled it. | MED: PEP 440 grammar edge; small source change. | LOW | candidate |
| 4 | pypa/packaging#1295 | 2026-06-26 | Unioning VersionRange objects that collapse to the full range dropped the autodetected prerelease policy. | HIGH but NICHE: subtle set-algebra + policy preservation on a brand-new API. | MED (niche API, possible ambiguity) | candidate (stretch) |
| 5 | pypa/packaging#1305 | 2026-06-29 | A wheel/sdist filename with an empty project name was accepted; it should be rejected. | LOW: add a guard. Likely aced. | LOW | rejected-easy (provisional) |
| 6 | more-itertools/more-itertools#1193 | 2026-06-30 | interleave_evenly on empty input errored; should return empty. | LOW: empty-input guard. Likely aced. | LOW | rejected-easy (provisional) |

## Next sweeps (to grow the pool toward ~25 candidates -> keep 10-15 admitted)
Target more low-dep pure-Python repos with strong test suites and subtle-bug surface:
httpx, starlette, urllib3, jsonschema (validators), tomlkit (python-poetry/tomlkit),
dateutil, rich, pygments, sqlglot, sympy, networkx, pyparsing, lark, babel, humanize,
python-dateutil, croniter, semver, jsonpatch, deepdiff. Prefer parser/protocol/date/
version/set-algebra bugs where the obvious fix is incomplete.

## Batch 2 (swept 2026-06-30: app-scale repos; post-cutoff >= 2026-02-01)
Build tier = how hard the test env is to stand up. Difficulty = provisional.

| PR | merged | task (paraphrased) | difficulty | build | status |
|----|--------|--------------------|-----------|-------|--------|
| tobymao/sqlglot#7807 | 2026-06-29 | Parser wrongly parses a parenthesized single expression where a Tuple is expected when a multi-expression CSV exists nearby; disambiguate. | HARD | light | candidate |
| tobymao/sqlglot#7813 | 2026-06-30 | Make Postgres date-format parsing robust to more formats. | MED-HARD | light | candidate |
| tobymao/sqlglot#7804 | 2026-06-30 | RegexpReplace type inference returns UNKNOWN on Hive/Spark/Databricks; infer correctly. | MED | light | candidate |
| tobymao/sqlglot#7808 | 2026-06-30 | Presto: transpile FROM_ISO8601_TIMESTAMP_NANOS to a cast. | MED | light | candidate |
| pallets/flask#5928 | 2026-02-20 | All teardown callbacks must run even if one raises; a raising callback skipped the rest. | MED | light | candidate |
| pallets/flask#5917 | 2026-02-12 | provide_automatic_options override was not respected on a route. | MED | light | candidate |
| encode/django-rest-framework#9977 | 2026-06-10 | OpenAPI schema drops zero numeric bounds (min/max = 0) on ListField children (falsy-zero bug). | MED | medium | candidate |
| encode/django-rest-framework#9973 | 2026-06-09 | Serializer crashes on an unhashable BooleanField representation value. | MED | medium | candidate |
| encode/django-rest-framework#9934 | 2026-05-13 | Browsable API breaks for a viewset extra action. | MED-HARD | medium | candidate |
| aio-libs/aiohttp#13016 | 2026-06-30 | Reading the body after a failed WebSocket upgrade misbehaves. | HARD | medium | candidate |
| aio-libs/aiohttp#12997 | 2026-06-27 | parse_content_disposition rejects headers with optional whitespace around the disposition type. | MED-HARD | medium | candidate |
| pytest-dev/pytest#14639 | 2026-06-27 | Off-by-one in trailing assertion-diff skipping. | MED | medium | candidate |
| pytest-dev/pytest#14646 | 2026-06-26 | Error on top-level options in pytest.toml config. | MED | medium | candidate |
| python-poetry/poetry#10943 | 2026-06-13 | Dependency resolver raises a spurious empty-constraint conflict when a package is duplicated across extras. | HARD | heavy | candidate |

Note: sqlglot#7809 (chained range operators) is tagged [CLAUDE] = LLM-assisted PR. Usable but note provenance.

## Recommended first build set (spans difficulty + build tier + hard tail)
sqlglot#7807 (hard/light), sqlglot#7813 (med-hard/light), flask#5928 (med/light),
DRF#9977 (med/medium), aiohttp#12997 (med-hard/medium), poetry#10943 (hard/heavy).

## Size-informed re-rank (2026-06-30)
Patch size + file spread is a difficulty signal (v1 median 18 LOC and it saturated).
DROP as likely-aced (too small): sqlglot#7807 (+2/-2), #7813 (+5), #7804 (+7),
DRF#9977 is borderline (+10). Favor large/multi-file/complex-domain for the hard tail.
REVISED first build set: flask#5928 (+194/10f, teardown, light), poetry#10943 (resolver, heavy),
DRF#9934 (+64 renderers, medium), aiohttp#12997 (multipart OWS, medium), sqlglot#7808 (+20/4f, light),
DRF#9973 (+39 fields, medium). Building flask#5928 first as the template.

## Build log
- 2026-06-30: PIPELINE PROVEN on sqlglot#7808. Method: clone, base = mergeCommit^1, bring in
  the PR's test file, run only the changed test file (avoids unrelated optional-dep collection
  errors). Result: BASE fails tests/dialects/test_presto.py::TestPresto::test_cast (the new
  FROM_ISO8601_TIMESTAMP_NANOS -> snowflake/spark/databricks/bigquery subtests); GOLD passes
  (20 passed, 527 subtests). Deterministic, low-dep. sqlglot#7808 status -> validated-mechanics
  (MED difficulty, UNMEASURED). Next: formalize into tasks/v2/ (self-contained oss_tests.py like
  the oss-click template, since sqlglot's own tests import a Validator base), then measure.
- Env gotchas for the recipe: macOS ships bash 3.2 (no `mapfile`); the harness shell is zsh (no
  unquoted word-split). Use portable file-based loops. Run only the PR's changed test file, not
  the whole suite (optional deps like pandas/duckdb break unrelated collection).

## Build log (2026-06-30 overnight)
PIPELINE PROVEN end to end. Triage of batch-1 low-dep candidates: REPRO = sqlglot#7808,
packaging#1295/#1299/#1305, more-itertools#1193. app-scale (flask/DRF) = env-issue (need
Django/flask test deps); sqlglot#7807/#7804/#7813 dropped (too small / .sql-fixture tests).
BUILT + HARNESS-VALIDATED (gold=1.0/base=0.0/deterministic in docker), all zero-dep:
  - oss-sqlglot-iso8601-nanos  (sqlglot#7808, Presto FROM_ISO8601_TIMESTAMP_NANOS -> cast)
  - oss-more-itertools-interleave-empty  (#1193, easy)
  - oss-packaging-empty-name  (#1305, easy)
NOT built: packaging#1299 (behavior does not discriminate on obvious inputs; subtle error-msg
change), packaging#1295 (niche VersionRange API, no public prerelease-policy attr -> risky to
author self-contained; best low-dep HARD candidate, revisit via -k real-test-file), flask/DRF/
aiohttp/poetry (need per-task Docker dep envs = the hard-tail work for next session).
Difficulty measurement (Sonnet5 + Opus4.8, high effort, repeat 3, -o ../vb-v2): RUNNING.
Helpers: scratchpad/{triage_pr.py, formalize.py, prep_pkg.py}; per-task cfg_*.json.

## MEASURED RESULT (2026-06-30 overnight, Sonnet5 + Opus4.8, high effort, repeat 3, 24 runs, $25.77)
| task | Sonnet 5 | Opus 4.8 |
|------|----------|----------|
| oss-more-itertools-interleave-empty | 100% $0.41 | 100% $0.28 |
| oss-packaging-empty-name | 100% $0.81 | 100% $1.59 |
| oss-packaging-range-prerelease-policy (HARD candidate) | 100% $3.51 | 100% $1.94 |
| oss-sqlglot-iso8601-nanos | 100% $15.15 | 100% $2.08 |
| **AGGREGATE** | **100%** | **100%** |
NO hard tail: both frontier models ace all 4, incl. the subtle VersionRange policy bug. Cleanly
buildable low-dep bugs are the wrong difficulty. Cost signal real: Sonnet $19.88 vs Opus $5.89
(Sonnet thrashes on the 76k-LOC sqlglot repo, ~7x). Hard tail needs app-scale dep-env tasks or
larger messier multi-file PRs (next session).

## 2026-07-01 session
- HARNESS CACHING added (harness/agent/providers.py): prompt-cache breakpoints on tools+system
  and the growing transcript tail, so the agent loop's re-sent context bills at cache-read rates.
  Verified (provider tests pass, breakpoints placed correctly). Cuts cost going forward; NOT yet
  demonstrated live (credits ran out).
- NEW HARD CANDIDATE BUILT + HARNESS-VALID: oss-sqlglot-qualify-lateral-star (sqlglot#7802,
  optimizer). Bug: qualify does not expand SELECT * to include a CROSS JOIN LATERAL subquery's
  columns, and mishandles partial column-alias lists. Optimizer-depth (column scope resolution),
  not a one-liner. gold=1.0/base=0.0/deterministic in docker. PENDING difficulty screen (blocked
  on API credits). Now in suite.json (5 tasks).
- BLOCKER: Anthropic API credits EXHAUSTED ("credit balance too low"). Screening + further hard
  builds need a top-up. With caching now on, credits stretch much further per run.
- Next hard candidates identified (sweep of complex zero/low-dep repos): sqlglot#7819 (optimizer
  qualified-star canonical names), sympy#29869 (elliptic curve P+(-P)), sympy#29939 (assumptions),
  lark#1577/#1601 (Earley parser ambiguity/SPPF). Optimizer/parser/symbolic-math = the domains
  most likely to trip the frontier; build + screen when credits return.

## BREAKTHROUGH (2026-07-01, screen repeat 1, high effort)
oss-sqlglot-qualify-lateral-star DISCRIMINATES THE FRONTIER (first v2 task not all models get):
  - Opus 4.8: PASS (functional=1.0), 134 steps, $0.56, 131s
  - Sonnet 5: FAIL (functional=0.0), 318 steps, 415k tok, 838s (14 min), $2.06
Optimizer-DEPTH (CROSS JOIN LATERAL column/scope resolution) is the vein: Sonnet flailed 318
steps and could NOT solve it; Opus solved it in 134. n=1 -> confirm at repeat 3.
CACHING CONFIRMED WORKING: per-turn effective prompt_tokens bounded (~1-10k across 78 turns,
max 10.7k) vs uncached linear growth; ~4-5x fewer billable input tokens (uncached iso run 1.9M
vs cached 415k, comparable step counts). Trace stores folded effective tokens, not raw cache
fields. PLAN: hand-build ~8-10 more optimizer/parser/depth tasks (manual public-API probing,
auto-triage can't isolate fixture/parametrized suites). Target mix for 80-90% aggregate: split
tasks (Sonnet-fail/Opus-pass), some both-pass (medium), maybe some both-fail.

## Screen batch 2 (2026-07-01, repeat 2, high effort, caching on)
| task | Sonnet 5 | Opus 4.8 |
|------|----------|----------|
| oss-sqlglot-qualify-lateral-star | 0% (0/2) $1.25 | 100% (2/2) $1.21 |  <- ROBUST SPLIT (Sonnet 0/3 total, Opus 3/3)
| oss-sqlglot-parser-tuple-subquery | 100% (2/2) $0.22 | 100% (2/2) $0.21 |  aced
| oss-sqlglot-hive-json-roundtrip | 100% (2/2) $2.28 | 100% (2/2) $0.63 |  aced
SHARPENED VEIN: the discriminator is specifically OPTIMIZER COLUMN-SCOPE RESOLUTION (qualify/
resolver/scope across lateral joins), NOT parser disambiguation and NOT dialect function
mapping (both aced). To build more Sonnet-discriminators -> target sqlglot optimizer qualify/
scope/resolver bugs. To trip OPUS too (needed for both models <90%) -> hardest optimizer bugs
or app-scale env tasks. Current 7-task suite: Sonnet 85.7% (6/7), Opus 100% (7/7). Caching
confirmed again (parser-tuple $0.22 both; lateral Sonnet $1.25 vs ~$5 uncached iso task).

## 2026-07-01 built + pre-vetted (reliable method: read merge test method, lift exact
## query+schema+expected, probe base-vs-gold in isolation, author self-contained oss_tests.py)

BUILT + DOCKER-VALID (added to suite, now 9 tasks; screening for frontier discrimination in flight):
- oss-sqlglot-resolver-flatten-recursion  #7737 resolver.py  ~12L  CRASH flavor (LATERAL FLATTEN
  over table missing from schema: base RecursionError, gold clean OptimizeError)
- oss-sqlglot-star-cte-struct-shadow  #7718 qualify_columns.py  ~6L  WRONG-OUTPUT flavor (BigQuery
  unreferenced CTE must not shadow a STRUCT column in `one.*` star expansion)

PRE-VETTED, buildable, NOT yet built (build if screen confirms the vein discriminates the frontier):
- #7550  qualify UNPIVOT on CTE sources  (qualify_columns+resolver+parser, MULTI-FILE = likely
  harder)  base e3fee4d2dc merge 8f572f8656  WRONG-OUTPUT (qualify().sql())
- #7614  avoid pushdown projections for column-dependent funcs (OBJECT_CONSTRUCT(*))  (pushdown_
  projections+scope)  WRONG-OUTPUT (optimize().sql())
LESS CLEAN (scope-internals / type-annotation / weaker decontam, deprioritized): #7549 (EXPLODE
qualify+annotate, overlaps existing lateral tasks), #7535 (scope UDTF, fixture), #7343 (correlated
subquery, asserts on traverse_scope external_columns = scope internals, March=weaker decontam).

## 2026-07-01 poetry app-scale triage (Opus-tail / cross-repo variety path)
poetry#10943 "avoid spurious empty-constraint conflict with duplicates across extras" (dep
resolver), merged 2026-06-13, 2 files: src/poetry/puzzle/provider.py + tests/puzzle/test_solver.py.
base d57090395b14ba260a24a97ab704819667bf6cf1  merge 9be6dd6e75906312c5dc477907e6901fa81ae024.
HEAVY ENV CONFIRMED: test_solver.py = 5319 lines, imports cleo + poetry-core + factory + full
Solver/Repository stack, fixture-dependent (conftest tree). Needs a PER-TASK Docker image
(pip install poetry + deps) + the whole tests/ conftest tree + likely -k selection (self-
contained authoring of a solver/extras scenario is very involved). Multi-hour build, NOT quick.
Decision gate: only invest if sqlglot multi-file bugs (e.g. unpivot-cte-star #7550) can't trip
Opus; clone cached at scratchpad/build/poetry.

## 2026-07-01 pm: app-scale per-task-image recipe PROVEN; Opus-tail hunt status
Per-task Docker images unblock app-scale tasks. Recipe (see sandbox/Dockerfile.poetry-10943,
Dockerfile.flask-5928): FROM vulcanbench/sandbox:base, pip install <pkg>@<base commit> (brings the
full dep stack), pip uninstall -y <pkg>, leave a stub dist-info so importlib.metadata.version()
works, run hidden tests with PYTHONPATH=src. Neutralize repo pytest config in graded cmds with
`-o addopts=` (poetry's pyproject injects `-n logical` = pytest-xdist; same landmine class as the
v1 coverage gate).

- poetry#10943 BUILT + DOCKER-VALID as oss-poetry-extras-duplicate-conflict (resolver, per-task
  image). SCREEN: Opus 4.8 PASSED (94 steps, $0.43). The dep-resolver/app-scale-env bet did NOT
  trip Opus; difficulty was never the env. Sonnet screen pending.
- flask#5928 BUILT + DOCKER-VALID as oss-flask-teardown-robust (multi-file robustness redesign,
  +80 src lines across app.py/ctx.py/helpers.py, ExceptionGroup aggregation; hidden test asserts
  structure not message wording). Screening both models.
- lark#1577/#1601 TRIAGED + DROPPED: 10-line and 6-line single-file xearley.py fixes; the
  clean-single-location shape that always gets aced.
Remaining untested Opus-tail shapes: large feature PRs graded on full parity, aiohttp#13016
(Cython+async), spec-realistic tasks graded against maintainer behavior.

## 2026-07-01 CONFIRMED: first both-fail task
oss-flask-teardown-robust (flask#5928): Opus 4.8 **0/3**, Sonnet 5 **0/3** (repeat-3 confirm,
$4.64; 0/4 each incl. screens). Opus consistently fixes do_teardown_request but leaves
do_teardown_appcontext/signal send un-robust; Sonnet thrashes ~300+ steps, often no patch.
THE OPUS-TRIPPER SHAPE: multi-site behavioral redesign with an explicit contract, graded on
full structural parity. poetry#10943 screen: Opus PASS / Sonnet FAIL -> 5th Sonnet-only
discriminator; app-scale env is NOT what makes tasks hard.
NEXT BUILDS for a both-models-in-band ~14-task composition (Sonnet ~71%, Opus ~86%):
- 3 pass-both anchors (safe, no screen needed): e.g. click#3582, packaging#1299 revisit,
  sqlglot#7804/#7813 smalls.
- 1 more flask-shaped both-fail candidate: DRF#9934 (browsable API renderers, +64),
  aiohttp#13016 (ws upgrade body, Cython), sqlglot#7720 (4-file feature). Screen ~$2.
- Then trim Sonnet-only discriminators from 5 -> 2 (keep lateral-star + poetry for repo
  diversity; retire or bench the other 3 as spares).

## 2026-07-01 FINAL COMPOSITION (suite.json locked, 10 tasks)
ilike-rename screen: Opus PASS $0.50/134st, Sonnet PASS $2.38/442st -> brutal cost-signal
hard anchor. aiohttp screen: both pass (Sonnet thrash). Opus is only tripped by flask ->
1 both-fail task forces N=10 (Opus (N-1)/N <= 90%).
FINAL 10 (7 repos, Python+Go): flask-teardown (both-fail), qualify-lateral-star (Sonnet-only
0/4), aiohttp-upgrade-deferred + star-ilike-rename + star-cte-struct-shadow (hard anchors,
cost discrimination), click-version-distribution-name, chi-readfrom-tee-doublecount (Go),
more-itertools-interleave-empty, packaging-range-prerelease-policy, packaging-empty-name.
Projected Sonnet 80% / Opus 90%.
SPARES (validated, rotate to counter overfitting; also future Opus-tripper builds go here
first): iso8601-nanos, parser-tuple-subquery, hive-json-roundtrip, resolver-flatten-recursion,
unpivot-cte-star, explode-struct-resolve, poetry-extras-duplicate-conflict.
PENDING: final 2-model repeat-1 (or repeat-3, ~$35) measurement after credit top-up;
TS-language task = known gap (node strip-types constraint makes real TS repos awkward).
