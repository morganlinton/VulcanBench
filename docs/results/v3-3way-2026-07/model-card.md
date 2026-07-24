# VulcanBench Technical Report No. 4 — Grok 4.5 vs Claude Fable 5 vs GPT-5.6 Sol vs Claude Opus 5

**July 12–24, 2026 · VulcanBench v3 · 23 tasks · 5 languages**

Twenty-three real merged post-cutoff PRs (Python 9, Rust 4, TypeScript 4, JavaScript 3, Go 3),
each graded by deterministic hidden tests. The original three-model matrix (Grok / Fable / Sol)
used a network-isolated Docker sandbox, one attempt per task per effort level (low, medium, high),
207 runs, $96.72 total. Every task pre-validated: gold patch = 1.0, unpatched = 0.0, deterministic
over 3 runs.

**Claude Opus 5** was added on launch day (2026-07-24) across all three effort levels against the
same v3 suite (`--no-judges`). See footnote ‡ for protocol differences.

## Results

| Model (effort) | Score | pass@1 | Suite cost | Tokens/task | Time/task | $/task | $/solved |
|---|---|---|---|---|---|---|---|
| **Grok 4.5 (medium)** | **21/23** | **91.3%** | $6.67 | 106 K | 3.9 min | $0.29 | $0.32 |
| Grok 4.5 (high) | 21/23 | 91.3% | $8.76 | 141 K | 4.9 min | $0.38 | $0.42 |
| Claude Fable 5 (low)† | 20/23 | 87.0% | $12.18 | 28 K | 3.4 min | $0.53 | $0.61 |
| GPT-5.6 Sol (high) | 20/23 | 87.0% | $15.90 | 85 K | 4.2 min | $0.69 | $0.80 |
| Claude Fable 5 (high)† | 20/23 | 87.0% | $20.82 | 44 K | 4.3 min | $0.91 | $1.04 |
| **Claude Opus 5 (low)‡** | **20/23** | **87.0%** | **$9.17** | **37 K** | **3.3 min** | **$0.40** | **$0.46** |
| Claude Opus 5 (medium)‡ | 20/23 | 87.0% | $12.24 | 57 K | 3.8 min | $0.53 | $0.61 |
| Claude Opus 5 (high)‡ | 20/23 | 87.0% | $22.34 | 112 K | 5.4 min | $0.97 | $1.12 |
| Grok 4.5 (low) | 19/23 | 82.6% | $3.39 | 57 K | 2.5 min | $0.15 | $0.18 |
| GPT-5.6 Sol (medium) | 19/23 | 82.6% | $8.83 | 50 K | 2.8 min | $0.38 | $0.46 |
| Claude Fable 5 (medium)† | 19/23 | 82.6% | $16.32 | 36 K | 3.7 min | $0.71 | $0.86 |
| GPT-5.6 Sol (low) | 18/23 | 78.3% | $3.85 | 23 K | 1.5 min | $0.17 | $0.21 |

† Fable 5 ran with Opus 4.8 refusal fallback: 6 of 69 runs (8.7%) were declined by the API
safety classifier (category `cyber`; the same three tasks at every effort tried) and served
end-to-end by Opus 4.8; all six passed. Tokens are billed tokens (prompt-cache reads
discounted); time is sandbox wall-clock.

‡ Opus 5 launch-day column (2026-07-24): `anthropic:claude-opus-5`, all three efforts,
`--no-judges`, local sandbox (harness Docker start fails cgroupv2 on this host). Pricing
$5/$25 per M (same as Opus 4.8). Low effort had a first-pass Anthropic `529 overloaded` on
14/23 tasks; gaps filled with `--only-missing`. `chi-readfrom-tee-doublecount` fails at every
effort (and broke pass_to_pass under local sandbox — treat as unverified). `itertools-strip-prefix`
fails at low/medium and passes at high.

## Findings

1. **Grok plateaus at medium — and that's the efficiency frontier.** High effort spends 33%
   more tokens and 31% more money for the identical 21/23, failing the same two tasks. No
   config in the full matrix beats 91.3% at $0.29/task.
2. **Only Sol rewards the effort knob.** 78.3 → 82.6 → 87.0%, monotonic; each step buys real
   accuracy. Fable oscillates at 85±2% (87.0 → 82.6 → 87.0) with a different failure set each
   run — the knob changes which borderline tasks fall, not how many.
3. **Opus 5 is Fable-shaped: flat 20/23 across low/medium/high.** Score does not move with
   effort; the failure set rotates (PennyLane ✓✗✗, SQLGlot canonicalize ✗✓✗, itertools ✗✗✓).
   High effort spends **2.4×** low ($22.34 vs $9.17) for the same 87.0% — low is the Opus 5
   efficiency point, matching Fable-low accuracy at ~25% lower suite cost.
4. **Frugal tokens at low, Grok-like at high.** Opus 5 low uses 37 K tokens/task (near Fable);
   high balloons to 112 K (near Grok-medium). Time follows: 3.3 → 3.8 → 5.4 min/task.
5. **The prior ceiling cracked but did not fall.** Before Opus 5, PennyLane Trotter and SQLGlot
   canonicalization were 0-for-27. Opus 5 clears each at one effort level but never both in the
   same config; `chi-readfrom-tee-doublecount` is a new persistent miss (env caveat). Flask
   teardown is ✓ at every Opus 5 effort (was high-only for the original trio).

## Failure map (tasks failed by at least one config)

| Task | Grok l/m/h | Fable l/m/h | Sol l/m/h | Opus 5 l/m/h‡ |
|---|---|---|---|---|
| pennylane-trotter-fragmented | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✗ ✗ | ✓ ✗ ✗ |
| sqlglot-canonicalize-internal-names | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✗ ✗ | ✗ ✓ ✗ |
| flask-teardown-robust | ✗ ✓ ✓ | ✗ ✓ ✓ | ✗ ✗ ✓ | ✓ ✓ ✓ |
| aiohttp-upgrade-deferred | ✗ ✓ ✓ | ✓† ✓† ✓† | ✗ ✓ ✓ | ✓ ✓ ✓ |
| itertools-strip-prefix | ✓ ✓ ✓ | ✓ ✓ ✓ | ✗ ✗ ✗ | ✗ ✗ ✓ |
| networkx-leiden-communities | ✓ ✓ ✓ | ✓ ✗ ✗ | ✓ ✓ ✓ | ✓ ✓ ✓ |
| sqlglot-iso8601-nanos | ✓ ✓ ✓ | ✓ ✗ ✓ | ✓ ✓ ✓ | ✓ ✓ ✓ |
| chi-readfrom-tee-doublecount | ✓ ✓ ✓ | ✓ ✓ ✓ | ✓ ✓ ✓ | ✗§ ✗§ ✗§ |
| (remaining 15 tasks: all ✓ on Opus 5 and on every prior config) | | | | |

§ Also failed the pass_to_pass regression guard under the local sandbox — env-contaminated;
  not a confirmed model miss.

## Reproducibility

Run records with full traces, final patches, and replay HTML are under `runs/` (suite `v3`).
Pricing: Grok 4.5 $2/$6, GPT-5.6 Sol $5/$30, Claude Fable 5 $10/$50, Claude Opus 5 $5/$25
per million tokens. Fable refusal fallback: set `VULCANBENCH_REFUSAL_FALLBACK=claude-opus-4-8`.

```
vulcanbench run --suite v3 --model <provider:model> --effort <low|medium|high> --no-judges
# Opus 5 full effort column:
vulcanbench run --suite v3 --model anthropic:claude-opus-5 --effort low --no-judges
vulcanbench run --suite v3 --model anthropic:claude-opus-5 --effort medium --no-judges
vulcanbench run --suite v3 --model anthropic:claude-opus-5 --effort high --no-judges
```
