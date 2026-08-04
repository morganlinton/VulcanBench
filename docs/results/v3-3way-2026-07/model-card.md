# VulcanBench Technical Report No. 4 — fair 10-task slice (post–Opus 5 cutoff)

**July 12–24, 2026 · VulcanBench v3 clean subset · 10 tasks · 5 languages**

> **Why 10 tasks.** Claude Opus 5’s training cutoff is **May 2026** ([Anthropic](https://support.claude.com/en/articles/8114494-how-up-to-date-is-claude-s-training-data)).
> **13 of 23** v3 tasks merge on or before 2026-05-31 and may be in Opus 5 training data.
> This card compares **all four models on the same 10 post-cutoff tasks only** — a fair
> head-to-head. Full 23-task suite numbers are contaminated for Opus 5 and are not the
> headline here. See [CONTAMINATION.md](./CONTAMINATION.md).

Claude Fable 5’s published cutoff is **January 2026** (all 23 merges are later by date).
Grok / Sol cutoffs not verified; they are scored on the same 10 for parity with Opus 5.

## Fair results — 10 tasks merged after May 2026

| Model (effort) | Score | pass@1 | Failures on this slice |
|---|---|---|---|
| **Claude Opus 5 (low)‡** | **10/10** | **100%** | — |
| Grok 4.5 (medium) | 9/10 | 90% | pennylane |
| Grok 4.5 (high) | 9/10 | 90% | pennylane |
| Claude Fable 5 (low)† | 9/10 | 90% | pennylane |
| Claude Fable 5 (high)† | 9/10 | 90% | pennylane |
| GPT-5.6 Sol (medium) | 9/10 | 90% | pennylane |
| GPT-5.6 Sol (high) | 9/10 | 90% | pennylane |
| Claude Opus 5 (medium)‡ | 9/10 | 90% | pennylane |
| Claude Opus 5 (high)‡ | 9/10 | 90% | pennylane |
| Grok 4.5 (low) | 8/10 | 80% | pennylane, aiohttp |
| Claude Fable 5 (medium)† | 8/10 | 80% | pennylane, iso8601 |
| GPT-5.6 Sol (low) | 8/10 | 80% | pennylane, aiohttp |

### Opus 5 cost on the clean 10 (local-sandbox smoke)

| Effort | Score | Cost | Tokens/task | Time/task | $/solved |
|---|---|---|---|---|---|
| **low** | **10/10** | **$3.53** | 35 K | 3.1 min | **$0.35** |
| medium | 9/10 | $5.84 | 60 K | 3.6 min | $0.65 |
| high | 9/10 | $8.57 | 100 K | 4.5 min | $0.95 |

Per-task spends for the original Grok / Fable / Sol Docker matrix were not archived at
task grain in-repo, so their clean-subset **costs are not recomputed** here — only scores
(from the published failure map) are compared.

## The 10 tasks

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

## Findings

1. **On a fair slice, Opus 5 low is alone at 10/10.** Every other config tops out at 9/10
   (all miss pennylane). The contaminated full-suite story (“flat 20/23 like Fable”) hid this.
2. **PennyLane is the clean-slice ceiling.** Merged 2026-06-02 — post-cutoff for Opus 5.
   Only Opus 5 low clears it; medium/high Opus and all Grok / Fable / Sol efforts fail it.
3. **Low effort wins for Opus 5 on both score and cost** ($3.53 vs $5.84 / $8.57). Raising
   effort loses pennylane and roughly doubles spend.
4. **Grok / Sol still need medium+ for aiohttp** on this slice (low → 8/10). Fable medium
   uniquely drops iso8601 (oscillation), same pattern as the full suite.

## Failure map (clean 10 only)

| Task | Grok l/m/h | Fable l/m/h | Sol l/m/h | Opus 5 l/m/h‡ |
|---|---|---|---|---|
| pennylane-trotter-fragmented | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✗ ✗ | ✓ ✗ ✗ |
| aiohttp-upgrade-deferred | ✗ ✓ ✓ | ✓† ✓† ✓† | ✗ ✓ ✓ | ✓ ✓ ✓ |
| sqlglot-iso8601-nanos | ✓ ✓ ✓ | ✓ ✗ ✓ | ✓ ✓ ✓ | ✓ ✓ ✓ |
| (other 7 tasks: ✓ everywhere) | | | | |

## Contaminated tasks excluded (13, merged ≤2026-05-31)

flask-teardown-robust, jiff×3, zod×2, cobra-noduplicateargs, sqlglot-canonicalize-internal-names,
semver-truncate, networkx-leiden-communities, chi-readfrom-tee-doublecount, hono-request-bytes,
itertools-strip-prefix.

## Protocol notes

† Fable 5 with Opus 4.8 refusal fallback on 6/69 full-suite runs (aiohttp among them — all
passed via fallback). Scores above use the published matrix outcomes.

‡ Opus 5 launch-day smoke (2026-07-24): `anthropic:claude-opus-5`, `--no-judges`, local sandbox
(harness Docker start fails cgroupv2 here), $5/$25 per M. Low first-pass Anthropic 529s filled
via `--only-missing`. Grok / Fable / Sol: Docker sandbox matrix from Report No. 4.

```
# Fair slice = the 10 tasks listed above
vulcanbench run --suite v3 --model anthropic:claude-opus-5 --effort low --no-judges
```
