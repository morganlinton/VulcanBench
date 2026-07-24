# v3 suite contamination vs Claude Opus 5

**Audit date:** 2026-07-24  
**Cutoff source:** [Anthropic Help Center — Claude training data cutoffs](https://support.claude.com/en/articles/8114494-how-up-to-date-is-claude-s-training-data)  
**Rule used:** task merge date ≤ **2026-05-31** → potentially in Opus 5 training data.

Claude Opus 5 training cutoff: **May 2026**.  
Claude Fable 5 training cutoff: **January 2026** (all 23 v3 merges are later → OK for Fable by date).

## Contaminated for Opus 5 (13)

| Task | Merged |
|---|---|
| oss-jiff-strftime-negpad | 2026-02-12 |
| oss-flask-teardown-robust | 2026-02-19 |
| oss-jiff-date-day-lt1 | 2026-02-22 |
| oss-jiff-signdur-panic | 2026-02-28 |
| oss-cobra-noduplicateargs | 2026-04-25 |
| oss-zod-invert-codec | 2026-04-28 |
| oss-zod-proto-catchall | 2026-04-29 |
| oss-sqlglot-canonicalize-internal-names | 2026-05-04 |
| oss-semver-truncate | 2026-05-07 |
| oss-networkx-leiden-communities | 2026-05-12 |
| oss-chi-readfrom-tee-doublecount | 2026-05-16 |
| oss-hono-request-bytes | 2026-05-16 |
| oss-itertools-strip-prefix | 2026-05-21 |

## Clean for Opus 5 (10)

| Task | Merged |
|---|---|
| oss-pennylane-trotter-fragmented | 2026-06-02 |
| oss-semver-inc-dotted-prerelease | 2026-06-04 |
| oss-semver-xrange-order | 2026-06-09 |
| oss-packaging-range-prerelease-policy | 2026-06-26 |
| oss-sqlglot-qualify-lateral-star | 2026-06-26 |
| oss-aiohttp-upgrade-deferred | 2026-06-30 |
| oss-more-itertools-interleave-empty | 2026-06-30 |
| oss-sqlglot-iso8601-nanos | 2026-06-30 |
| oss-pflag-uintslice-hex | 2026-07-02 |
| oss-hono-client-header-merge | 2026-07-07 |

## Opus 5 scores on clean subset only

| Effort | Score | Cost |
|---|---|---|
| low | **10/10** | $3.53 |
| medium | 9/10 (miss: pennylane) | $5.84 |
| high | 9/10 (miss: pennylane) | $8.57 |

Full-suite 20/23 figures for Opus 5 must not be treated as post-training signal.
