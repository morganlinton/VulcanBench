# v3 candidate pool (sourced 2026-07-08)

Target: 23 tasks = Python 9 (carry from v2) + Rust 4 + TypeScript 4 + JavaScript 3 + Go 3.
All new tasks: real merged PRs, merged after model training cutoffs, sliced at base commit.
Grading discipline (per OpenAI SWE-Bench-Pro audit): >=3 fail_to_pass tests, prompt-test
alignment (every example constant generated from gold), no overly-strict/underspecified prompts.

## Toolchain notes (network-off sandbox is the binding constraint)
- base image has: Node 22 (`node --experimental-strip-types --test`), Go. Rust via `:rust` image.
- We write our OWN hidden tests (as we do for Python), using `node:test` — NOT the repo's
  vitest/tap suites (avoids extra deps). Source must be importable under strip-types/require.
- TS PROVEN 2026-07-08: extensionless-import TS source (hono) runs via tsx pinned in the
  new `node-ts` image (sandbox/Dockerfile.node-ts); test cmd `tsx --test vb_x.test.ts`.
- RUST PROVEN 2026-07-08: `cargo vendor` -> vendor/ + .cargo/config.toml replace-source makes
  cargo build/test fully offline (verified `docker run --network none ... cargo build --offline`);
  trim [[bench]]/criterion to keep vendoring lean. Uses the existing `:rust` image.
- RUST zero-dep shortcut 2026-07-09 (jiff #527): when the fix lives in a crate's dependency-free
  core, skip vendoring entirely. Trim to a standalone single crate (drop [workspace] members,
  optional deps, dev-deps, existing [[test]]s) + minimal Cargo.toml, build `--no-default-features
  --features std`. Zero external crates -> `cargo test --offline` just works. Copy include_str!'d
  docs (CHANGELOG/COMPARE/DESIGN/PLATFORM.md) or lib.rs won't compile.

## JavaScript (need 3) — node-semver is the goldmine (zero-dep, pure logic)
- [x] node-semver #855  feat: add `truncate` function  (BUILT + VALIDATED as oss-semver-truncate) ***
- [x] node-semver #870  fix: increment dotted prerelease identifiers (BUILT+VALIDATED oss-semver-inc-dotted-prerelease) ***
- [x] node-semver #874  fix: reject numeric segments after x-ranges (BUILT+VALIDATED oss-semver-xrange-order) ***  [JS 3/3 DONE]
- [ ] node-semver #869  fix: strip build metadata before comparator trimming
- [ ] commander #2544   fix: accept uppercase E exponent in negative number args

## TypeScript (need 4) — zod v4 / hono, both zero-dep
- [x] zod #5770   invertCodec (BUILT+VALIDATED oss-zod-invert-codec, multi_file classic+mini) ***
- [ ] zod #5900   fix(v4): reject tuple holes before required defaults (edge case)
- [x] zod #5898   __proto__ catchall (BUILT+VALIDATED oss-zod-proto-catchall) ***
- [ ] zod #5869   fix: handle `z.union([])` and `z.xor([])`         (edge case)
- [x] hono #4921  feat(request): add `bytes()`  (BUILT + VALIDATED as oss-hono-request-bytes, image node-ts) ***
- [x] hono #5092  client header merge (BUILT+VALIDATED oss-hono-client-header-merge)

## Rust (need 4) — itertools (dep: either) / jiff (BurntSushi, dep-light core)
- [x] itertools #1104  strip_prefix (BUILT+VALIDATED oss-itertools-strip-prefix; vendored offline) ***
- [x] jiff #527        fix panic in `TryFrom<SignedDuration> for std::time::Duration` (BUILT+VALIDATED oss-jiff-signdur-panic; standalone crate, zero-dep offline via --no-default-features --features std, NO vendoring needed) ***
- [x] jiff #524        `Date::new` rejects day < 1 (BUILT+VALIDATED oss-jiff-date-day-lt1) ***
- [x] jiff #487        strftime negative-value padding (BUILT+VALIDATED oss-jiff-strftime-negpad) ***
- [~] jiff #570        DROPPED: `Debug for UnitSet` is pub(crate), not observable via public API -> no clean fail_to_pass
- [ ] clap #6414       fix(complete): don't emit usage placeholder for plain positionals
- [ ] clap #6340       fix(complete): do not suggest options after `--`
  (NOTE: clap completion output is harder to grade cleanly; prefer itertools/jiff)
  RUST 4/4 DONE: itertools #1104, jiff #527, jiff #524, jiff #487

## Go (need 2 new + carry chi)
- [x] cobra #2397  Add NoDuplicateArgs validator (BUILT+VALIDATED oss-cobra-noduplicateargs; NET-NEW; deps vendored via `go mod vendor`, cmd `go test -mod=vendor ./osscheck/`) ***
- [x] pflag #484   UintSlice accepts hex/octal/binary (BUILT oss-pflag-uintslice-hex; zero-dep, no vendor) *** [validating]
- [~] cobra #2356  DROPPED: mutation is in private getCompletions on internal args -> not cleanly observable via public API
- [x] chi-readfrom-tee-doublecount (CARRY from v2; f2p 1->3: added two-ReadFrom accumulate + Write-then-ReadFrom, both catch the double-count)
  GO RECIPE PROVEN 2026-07-09: base image has Go 1.23; hidden tests as a package under tests/osscheck/
  (-> ws/osscheck/) run via `go test ./osscheck/`. Deps: `go mod vendor` + cmd `go test -mod=vendor`
  (verified offline --network none, no GOPROXY). Zero-dep libs (pflag, chi) need no vendor. NET-NEW-symbol
  tasks: put pass_to_pass in a SEPARATE package (osscheck_reg) so it compiles+passes at base.

## Python (need 9) — CARRIED from v2 with the coverage-floor upgrade  [9/9 DONE]
- [x] flask-teardown-robust            f2p 1->3 (added appcontext-aggregate + partial-continue)
- [x] aiohttp-upgrade-deferred         f2p 2->3 (added split-across-feeds deferral)
- [x] sqlglot-qualify-lateral-star     f2p 2->4 (added single-col lateral + qualified star)
- [x] sqlglot-iso8601-nanos            f2p 1->3 (split by cast type + dedicated-expr node check)
- [x] packaging-range-prerelease-policy f2p 1->3 (wrote 3 vb_ union-with-full policy tests)
- [x] more-itertools-interleave-empty  f2p 1->3 (empty+lengths, immediate StopIteration)
- [x] networkx-leiden-communities      f2p=6 already above floor, carried as-is
- [x] sqlglot-canonicalize-internal-names f2p=6 already above floor, carried as-is
- [x] pennylane-trotter-fragmented     f2p=5 already above floor, carried as-is
  COVERAGE FLOOR: every v3 task now has >=3 fail_to_pass tests, each independently verified
  to fail at the base commit (probed base-vs-gold before writing each new test).

## STATUS 2026-07-09: v3 COMPLETE + SCREENED — 23/23 validated, suite.json written.
Haiku-first screening funnel ($14.58 total, budget $15-25):
- Haiku 4.5: 18/23 pass (all TS, all JS, all Go, 3/4 Rust, 6/9 Py). Every pass < $0.75.
- Opus 4.8 escalation on the 5 Haiku misses: itertools-strip-prefix 1.0 (Haiku's 0.0
  was a fair spec deviation: built 2-generic StripPrefixError vs the pinned 3-generic),
  semver-truncate 1.0, networkx-leiden 1.0 -> 21/23 confirmed model-solvable.
- Frontier-hard tail (both models fail, partial credit reachable): pennylane-trotter
  (0.2/0.2) and sqlglot-canonicalize (0.17/0.17) — the intended v2 veryhard carries,
  gold-validated, agents score partial f2p so grading is reachable, not broken.
Python 9 | Rust 4 | TypeScript 4 | JavaScript 3 | Go 3

Legend: *** = top pick (net-new or clean-testable). Screen every built task before counting it.
