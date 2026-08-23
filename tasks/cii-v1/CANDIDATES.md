# CII v1 — candidate worklog

Status date: 2026-08-22. Target: 30 all-new admitted tasks. See CHARTER.md for
composition targets and the admission gate.

## Mining sweeps

- **2026-08-22**: ~39 repos across Python/TS/JS/Go/Rust via `gh pr list`
  (merged >= 2026-07-01, >= 2 source files, test-bearing, 15–600 net adds,
  <= 25 files) → 72 candidates. Rust and JS came back thin (chrono, jiff,
  serde-json, itertools, semver, micromatch, minimatch, glob near-idle in the
  window) — the next sweep should widen those lists (candidates: rayon, regex,
  csv, rust-url, thiserror; js: undici, cheerio, dayjs, commander, yargs) and
  drop `--since` toward 2026-06-01 for those two languages only.

## Admitted (see suite.json)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-networkx-digraph-node-cuts | networkx#8837 | 2026-08-21 | py | multi_file/hard | five real algorithmic digraph bugs; stark symptoms |
| oss-toml-value-datetime-deserialize | toml#1194 | 2026-07-28 | rust | multi_file/medium | serde deserializer forwarding subtlety across 2 files |
| oss-cli-required-single-arg | cli#2393 | 2026-08-16 | go | system/medium | 4-file feature: API + parse + errors + usage rendering |
| oss-hono-regexp-wildcard-middleware | hono#5266 | 2026-08-19 | ts | multi_file/hard | router association logic; SmartRouter masks it |

### Wave 2 (2026-08-22, PRs verified vs both waves' dedup sweep)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-packaging-interpreter-tag-identifier | packaging#1351 | 2026-07-28 | py | multi_file/easy | deliberate easy anchor; vivid mis-parse symptom |
| oss-echo-problem-details | echo#3062 | 2026-07-30 | go | multi_file/medium | RFC 9457 feature with a rich behavioral contract |
| oss-zod-codepoint-length | zod#6441 | 2026-08-19 | ts | system/hard | 3 files incl. compiled codegen path; perf-aware fix |
| oss-clap-mangen-override-usage | clap#6467 | 2026-08-06 | rust | system/medium | cross-crate (clap_builder + clap_mangen) |
| oss-sqlglot-multi-table-ddl | sqlglot#8229 | 2026-08-20 | py | system/hard | 6-file breaking AST change; human-authored PR |

Wave-2 dedup catch: anyio#1228 and attrs#1592 were already python-1 tasks
(oss-anyio-fail-at-deadline, oss-attrs-generator-on-setattr) — the shortlist
below is re-checked, but always re-run the grep before building.

### Wave 3 (2026-08-22)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-chi-query-method | chi#1132 | 2026-07-05 | go | system/medium | HTTP QUERY across 3 files; bitmask 405 semantics |
| oss-zod-exactoptional-absent-key | zod#6434 | 2026-08-19 | ts | multi_file/hard | subtle optionality-ladder gate across interp+compiled |
| oss-eslint-property-descriptor-scope | eslint#21163 | 2026-07-28 | js | system/hard | FIRST JS task; scope-aware ast-utils across 4 files |
| oss-regex-static-macro | regex#1371 | 2026-07-09 | rust | system/medium | regex! macro feature; API-surface discipline |

Wave-3 infrastructure notes:
- **JS-with-deps pattern established**: `sandbox/Dockerfile.eslint-21163` (node-ts
  + `npm install --omit=dev` of the repo's package.json into /opt, NODE_PATH).
  Build with the TASK REPO as docker context — `.dockerignore` excludes `tasks/`
  from the root context.
- **autotests=false crates** (regex): the hidden-test overlay must also carry
  Cargo.toml with appended `[[test]]` targets; document that agent manifest
  edits are ignored at grading.
- Sweep #2 (JS/Rust, since 2026-06-01): rust-lang/cargo has task-shaped PRs but
  the repo is too heavy to sandbox; bitflags#489 (iter_equal_names) is a viable
  small Rust candidate; axios#11096/#11121 remain for JS (check test env needs).

### Wave 4 (2026-08-22)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-hono-trie-suffix-wildcard | hono#5236 | 2026-08-17 | ts | multi_file/medium | trie insertion bug only visible on SmartRouter fallback |
| oss-cli-arg-validator | cli#2377 | 2026-08-16 | go | system/medium | tree-wide validator hook; nearest-wins inheritance |
| oss-bitflags-iter-equal-names | bitflags#489 | 2026-06-05 | rust | multi_file/easy | second easy anchor; alias/convenience-name semantics |
| oss-axios-methodlist-adapter-errors | axios#11096 | 2026-08-13 | js | system/medium | shared method list + AxiosError adapter errors |

Wave-4 infrastructure notes:
- **ESM deps images** (axios): NODE_PATH does NOT apply to ESM imports — the
  per-task image symlinks `/node_modules` so the ancestor walk from the
  workspace mount finds the deps (`sandbox/Dockerfile.axios-11096`).
- Loopback servers work in the network-off sandbox (lo exists under
  `--network none`), so JS tasks can grade real request/response behavior.
- pydantic#13667 deferred: needs a per-task image pairing the in-tree
  pydantic-core python sources with a compiled _pydantic_core wheel — version
  pairing is the risk; attempt in wave 5.

- **Stale `target/` poisons host-side runs**: building in-place inside a task
  repo leaves same-platform artifacts that let `cargo test` reuse a PATCHED
  test binary at base (`--sandbox local` graded a mock run 1.0). Docker
  validation is immune (different platform), which is why the gate still held.
  Always `rm -rf repo/target` after in-place verification — or verify in a copy.

### Wave 5 (2026-08-22)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-anyio-create-task-names | anyio#1234 | 2026-07-19 | py | system/medium | naming parity across both backends; reuses anyio-1191 image |
| oss-echo-group-implicit-overwrite | echo#3049 | 2026-07-21 | go | system/medium | implicit-route bookkeeping vs overwrite protection |
| oss-eslint-loop-condition-ternary | eslint#21175 | 2026-08-11 | js | multi_file/medium | rule option feature; second eslint deps image |
| oss-pydantic-none-discriminator | pydantic#13667 | 2026-08-16 | py | multi_file/hard | monorepo overlay + pinned pydantic-core wheel (2.48.0) |

Wave-5 infrastructure notes:
- **pydantic monorepo pattern**: image bakes the exact pydantic-core wheel the
  base commit pins; the in-tree pydantic-core sources ship for navigation but
  are unimportable (hyphenated dir), so `import pydantic_core` is always the
  wheel. The fix's core_schema.py hunk is type-hints only (runtime-inert).
- Scope hidden tests to the upstream fix's actual contract: echo#3049 does NOT
  make explicit RouteNotFound override the implicit one (first test draft
  over-specified and failed under gold).
- eslint#21175: any options object fails at base (schema []) — an
  unknown-option-rejection test therefore passes at base too and belongs in
  the guards, not fail_to_pass.

### Wave 6 (2026-08-22)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-pygments-transparent-color | pygments#3180 | 2026-07-05 | py | multi_file/easy | third easy anchor; CSS keyword pass-through |
| oss-click-custom-version-option | click#3581 | 2026-07-08 | py | multi_file/medium | feature with frozen-API rationale |
| oss-jiff-offset-subtraction | jiff#617 | 2026-07-28 | rust | localized/easy | checked_sub ADDED; tests target jiff-core's public API |
| oss-ky-query-method | ky#873 | 2026-07-06 | ts | system/medium | QUERY shortcut + uppercasing + retry defaults |
| oss-undici-interceptors-origin | undici#5628 | 2026-08-01 | js | system/hard | silently-inert interceptors; hit-counting servers |

Wave-6 notes: sweep #3 (11 fresh repos/lang, since 2026-06-01) yielded 49
candidates with dedup flags. jiff's cargo-prune kept dependent integration
crates (diesel/sqlx/icu/wasm) — closure follows dependencies, so dependents
must be trimmed by hand before vendoring. The public jiff crate never calls
the buggy core routine: grade against jiff-core's own public API. Remaining
to 30: 4 tasks; viable leftovers include hono#5252/#5179, zod#6442,
body-parser#741, undici#5696, vitest mocker pair, trio (needs new PRs).

### Wave 7 — FINAL (2026-08-22)

| Task | Source PR | Merged | Lang | Complexity | Why |
|---|---|---|---|---|---|
| oss-hono-wildcard-prefix-overmatch | hono#5252 | 2026-08-17 | ts | multi_file/medium | prefix overmatch in Linear+Pattern routers |
| oss-bodyparser-limit-validation | body-parser#741 | 2026-07-07 | js | system/medium | fail-open limit misconfig; per-task deps image |
| oss-zod-url-ipv6-validated-string | zod#6442 | 2026-08-19 | ts | multi_file/hard | WHATWG-parser divergence; security-flavored |
| oss-undici-body-sent-hooks | undici#5696 | 2026-08-19 | js | system/medium | hooks dropped by every wrapped handler |

**SUITE COMPLETE: 30/30 admitted.** Final composition: Py 7 / TS 7 / Go 5 /
Rust 5 / JS 6; complexity 12x system, 13x multi_file, 1x localized anchor,
with 4 easy anchors total. Next step (needs API keys): the charter's
measure-then-compose pass — frontier repeat-3 over all 30 to calibrate the
difficulty band and identify discriminators.

## Triage rules learned

- **Dedup against every existing suite first** (`grep -rh '"url"' tasks/*/*/metadata.json`):
  werkzeug#3234 (etag discard) was already `vulcancyber-v1/oss-werkzeug-etag-strict-parse`.
- **Skip AI-authored upstream PRs** (sqlglot tags `[CLAUDE]`/`[CODEX]` in titles):
  a fix a model wrote is weak discrimination signal for models.
- **Skip deprecation-removal churn** (fastify FSTDEP series): mechanical deletions.
- Rust vendored slices: regenerate the lockfile for the pruned workspace before
  `cargo vendor`, and check the resulting dev-deps' MSRV — the toml slice needs
  rustc >= 1.88 (let-chains in `ignore`), hence sandbox:rust-2024.
- Go slices: strip the upstream `.gitignore`'s `vendor` rule (or `git add -f`);
  urfave/cli ships one.

## Strong remaining candidates (next waves)

- **python**: anyio#1228 (move_on_at/fail_at feature), networkx#8756 (edmonds_karp
  residual reset perf), attrs#1592 (on_setattr generators, 5 src), pydantic#13667
  (bare None in discriminated unions) / #13604 (PEP 695 callable discriminators)
- **typescript**: zod#6441 (code-point string length, 3 src), zod#6434 (middle-rung
  absent key), hono#5236 (trie suffix wildcard), nitro#4431 (bytes/text import
  attributes, 4 src + 7 test files)
- **javascript**: axios#11094 (xhr navigation-cancel → ECONNABORTED; needs jsdom-ish
  env — check testability), eslint#21163 (getter-return false positives, 4 src)
- **go**: echo#3049 (implicit group route overwrite), echo#3062 (RFC 9457 problem
  details, 2 src +323), cli#2377 (ArgValidator), chi#1132 (QUERY method)
- **rust**: clap#6467 (mangen SYNOPSIS override_usage, 3 src) — needs wider mining
  for more
