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
