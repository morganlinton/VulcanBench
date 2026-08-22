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
