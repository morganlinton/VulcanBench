# VulcanBench Technical Report No. 4 — Grok 4.5 vs Claude Fable 5 vs GPT-5.6 Sol vs Claude Opus 5

**July 12–24, 2026 · VulcanBench v3 · 23 tasks · 5 languages**

> **Contamination (Opus 5).** Anthropic documents Claude Opus 5 training data through
> **May 2026**. Of the 23 v3 tasks, **13** merge on or before 2026-05-31 and could have
> been in Opus 5 training data; only **10** are post-cutoff for Opus 5. Full-suite Opus 5
> scores below are therefore **contaminated** for that model. Prefer the clean-subset
> column. Claude Fable 5’s published cutoff is **January 2026** — by merge date, all 23
> tasks are post-cutoff for Fable. Grok / Sol cutoffs not verified here.

Twenty-three real merged OSS PRs (Python 9, Rust 4, TypeScript 4, JavaScript 3, Go 3),
each graded by deterministic hidden tests. The original three-model matrix (Grok / Fable / Sol)
used a network-isolated Docker sandbox, one attempt per task per effort level (low, medium, high),
207 runs, $96.72 total. Every task pre-validated: gold patch = 1.0, unpatched = 0.0, deterministic
over 3 runs.

**Claude Opus 5** was added on launch day (2026-07-24) across all three effort levels against the
same v3 suite (`--no-judges`). See footnote ‡.

## Results (full suite — Opus 5 contaminated)

| Model (effort) | Score | pass@1 | Suite cost | Tokens/task | Time/task | $/task | $/solved |
|---|---|---|---|---|---|---|---|
| **Grok 4.5 (medium)** | **21/23** | **91.3%** | $6.67 | 106 K | 3.9 min | $0.29 | $0.32 |
| Grok 4.5 (high) | 21/23 | 91.3% | $8.76 | 141 K | 4.9 min | $0.38 | $0.42 |
| Claude Fable 5 (low)† | 20/23 | 87.0% | $12.18 | 28 K | 3.4 min | $0.53 | $0.61 |
| GPT-5.6 Sol (high) | 20/23 | 87.0% | $15.90 | 85 K | 4.2 min | $0.69 | $0.80 |
| Claude Fable 5 (high)† | 20/23 | 87.0% | $20.82 | 44 K | 4.3 min | $0.91 | $1.04 |
| Claude Opus 5 (low)‡§ | 20/23 | 87.0% | $9.17 | 37 K | 3.3 min | $0.40 | $0.46 |
| Claude Opus 5 (medium)‡§ | 20/23 | 87.0% | $12.24 | 57 K | 3.8 min | $0.53 | $0.61 |
| Claude Opus 5 (high)‡§ | 20/23 | 87.0% | $22.34 | 112 K | 5.4 min | $0.97 | $1.12 |
| Grok 4.5 (low) | 19/23 | 82.6% | $3.39 | 57 K | 2.5 min | $0.15 | $0.18 |
| GPT-5.6 Sol (medium) | 19/23 | 82.6% | $8.83 | 50 K | 2.8 min | $0.38 | $0.46 |
| Claude Fable 5 (medium)† | 19/23 | 82.6% | $16.32 | 36 K | 3.7 min | $0.71 | $0.86 |
| GPT-5.6 Sol (low) | 18/23 | 78.3% | $3.85 | 23 K | 1.5 min | $0.17 | $0.21 |

## Opus 5 clean subset (10 tasks merged after May 2026)

| Effort | Score | pass@1 | Cost | Tokens/task | Time/task | $/task | $/solved | Fail |
|---|---|---|---|---|---|---|---|---|
| **low** | **10/10** | **100%** | $3.53 | 35 K | 3.1 min | $0.35 | $0.35 | — |
| medium | 9/10 | 90.0% | $5.84 | 60 K | 3.6 min | $0.58 | $0.65 | pennylane |
| high | 9/10 | 90.0% | $8.57 | 100 K | 4.5 min | $0.86 | $0.95 | pennylane |

On the uncontaminated slice, **low is both highest-scoring and cheapest** (10/10 for $3.53).
Medium/high drop pennylane and spend 1.7–2.4× more. Contaminated-slice misses (canonicalize,
itertools, chi) do not appear here — they are all pre-cutoff.

**Clean (10):** aiohttp-upgrade-deferred, hono-client-header-merge, more-itertools-interleave-empty,
packaging-range-prerelease-policy, pennylane-trotter-fragmented, pflag-uintslice-hex,
semver-inc-dotted-prerelease, semver-xrange-order, sqlglot-iso8601-nanos, sqlglot-qualify-lateral-star.

**Contaminated for Opus 5 (13, merged ≤2026-05-31):** chi-readfrom-tee-doublecount,
cobra-noduplicateargs, flask-teardown-robust, hono-request-bytes, itertools-strip-prefix,
jiff-date-day-lt1, jiff-signdur-panic, jiff-strftime-negpad, networkx-leiden-communities,
semver-truncate, sqlglot-canonicalize-internal-names, zod-invert-codec, zod-proto-catchall.

† Fable 5 ran with Opus 4.8 refusal fallback: 6 of 69 runs (8.7%) were declined by the API
safety classifier (category `cyber`; the same three tasks at every effort tried) and served
end-to-end by Opus 4.8; all six passed. Tokens are billed tokens (prompt-cache reads
discounted); time is sandbox wall-clock. Fable training cutoff Jan 2026 → all 23 merges are
post-cutoff by date.

‡ Opus 5 launch-day column (2026-07-24): `anthropic:claude-opus-5`, all three efforts,
`--no-judges`, local sandbox (harness Docker start fails cgroupv2 on this host). Pricing
$5/$25 per M. Low effort had a first-pass Anthropic `529 overloaded` on 14/23 tasks; gaps
filled with `--only-missing`. `chi-readfrom-tee-doublecount` fails at every effort (and broke
pass_to_pass under local sandbox — treat as unverified).

§ **Contaminated for Opus 5** — 13/23 tasks merge on or before the May 2026 training cutoff.
Do not treat full-suite Opus 5 pass@1 as a clean post-training signal.

## Findings

1. **Grok plateaus at medium — and that's the efficiency frontier** (full matrix, Docker).
   High effort spends 33% more tokens for the identical 21/23.
2. **Only Sol rewards the effort knob** on the full suite (78.3 → 82.6 → 87.0%).
3. **Opus 5 full-suite 20/23 is not trustworthy as a decontaminated score.** After dropping
   the 13 pre-May-2026 tasks, the remaining signal is **10/10 (low)** and **9/10 (medium/high)**
   — low wins on both accuracy and cost. The earlier “flat 20/23 like Fable” narrative does
   not survive the cutoff audit.
4. **Hard tail is post-cutoff.** `pennylane-trotter-fragmented` (merged 2026-06-02) is the only
   clean-subset miss at medium/high; low clears it. `sqlglot-canonicalize` is pre-cutoff for
   Opus 5 and must not be counted as a novel failure.
5. **Suite labeling debt.** Every v3 task still carries `decontaminated: false` with notes that
   claimed “after model training cutoffs.” Cutoffs are model-specific; Opus 5’s May 2026 line
   makes 13 of those notes false for that model.

## Failure map (tasks failed by at least one config)

| Task | Grok l/m/h | Fable l/m/h | Sol l/m/h | Opus 5 l/m/h‡ | Opus 5 clean? |
|---|---|---|---|---|---|
| pennylane-trotter-fragmented | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✗ ✗ | ✓ ✗ ✗ | yes (Jun 2) |
| sqlglot-canonicalize-internal-names | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✓ ✗ | **no** (May 4) |
| flask-teardown-robust | ✗ ✓ ✓ | ✗ ✓ ✓ | ✗ ✗ ✓ | ✓ ✓ ✓ | **no** (Feb 19) |
| aiohttp-upgrade-deferred | ✗ ✓ ✓ | ✓† ✓† ✓† | ✗ ✓ ✓ | ✓ ✓ ✓ | yes (Jun 30) |
| itertools-strip-prefix | ✓ ✓ ✓ | ✓ ✓ ✓ | ✗ ✗ ✗ | ✗ ✗ ✓ | **no** (May 21) |
| networkx-leiden-communities | ✓ ✓ ✓ | ✓ ✗ ✗ | ✓ ✓ ✓ | ✓ ✓ ✓ | **no** (May 12) |
| sqlglot-iso8601-nanos | ✓ ✓ ✓ | ✓ ✗ ✓ | ✓ ✓ ✓ | ✓ ✓ ✓ | yes (Jun 30) |
| chi-readfrom-tee-doublecount | ✓ ✓ ✓ | ✓ ✓ ✓ | ✓ ✓ ✓ | ✗¶ ✗¶ ✗¶ | **no** (May 16) |
| (remaining clean tasks: all ✓ on Opus 5) | | | | | |

¶ Also failed the pass_to_pass regression guard under the local sandbox — env-contaminated;
  not a confirmed model miss.

## Reproducibility

Run records with full traces, final patches, and replay HTML are under `runs/` (suite `v3`).
Pricing: Grok 4.5 $2/$6, GPT-5.6 Sol $5/$30, Claude Fable 5 $10/$50, Claude Opus 5 $5/$25
per million tokens. Fable refusal fallback: set `VULCANBENCH_REFUSAL_FALLBACK=claude-opus-4-8`.
Opus 5 cutoff source: [Anthropic Help Center — training data cutoffs](https://support.claude.com/en/articles/8114494-how-up-to-date-is-claude-s-training-data) (May 2026).

```
vulcanbench run --suite v3 --model <provider:model> --effort <low|medium|high> --no-judges
# Opus 5 — prefer interpreting only the 10 post-May-2026 tasks listed above
vulcanbench run --suite v3 --model anthropic:claude-opus-5 --effort low --no-judges
```
