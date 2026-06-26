# VulcanBench report: v2

_Generated 2026-06-26T14:03:09.781504+00:00_

- **315** runs · **3** models · **35** tasks · total cost $29.646438

> ⚠️ **9 run(s) scored against non-decontaminated task(s)** (oss-inflection-titleize) - these tasks derive from public sources that predate model training cutoffs; treat their scores with care.

## Models

| Model | Tasks | Runs | pass@1 ± se | pass@k | Avg total | Cost $ | Avg time |
|---|---|---|---|---|---|---|---|
| openai:gpt-5.5 | 35 | 105 | 1.0000 ± 0.0000 | 1.0000 | 0.9576 | 10.2088 | 44.7972 |
| anthropic:claude-opus-4-8 | 35 | 105 | 0.9714 ± 0.0286 | 0.9714 | 0.9329 | 11.6960 | 43.9060 |
| zai:glm-5.2 | 35 | 105 | 0.9048 ± 0.0376 | 0.9714 | 0.8444 | 7.7416 | 163.6856 |

## Discrimination

- **3 of 35** tasks separate at least one model pair (pass threshold 0.5). **32** carry no signal: 32 solved by every model, 0 failed by every model.

| Model A | Model B | Common tasks | Separating | A only | B only |
|---|---|---|---|---|---|
| anthropic:claude-opus-4-8 | openai:gpt-5.5 | 35 | 1 | 0 | 1 |
| anthropic:claude-opus-4-8 | zai:glm-5.2 | 35 | 3 | 2 | 1 |
| openai:gpt-5.5 | zai:glm-5.2 | 35 | 2 | 2 | 0 |

_Retirement candidates (no model separated by these 32 tasks): go-csv-quoting, go-lru-cache, go-money-allocate, go-parallel-map, go-stack-pop-bug, go-ttl-lru-cache, go-worker-pool, oss-go-errwrap-unwrap, oss-go-m2-01, oss-go-m3-04, oss-go-metrics-labels, oss-pilot-acme-retry, oss-py-config-merge, oss-py-ledger-rounding, oss-py-m2-00, oss-py-m3-00, oss-ts-router-params, oss-ts-validate-coerce, oss-ty-m2-02, oss-ty-m3-05, py-csv-export-feature, py-expr-eval, py-jsonpointer, py-retry-refactor, py-sliding-window-max, py-topo-sort-cycle, py-ttl-cache-expiry, rs-feature-gate, ts-deep-merge, ts-event-emitter, ts-querystring-bug, ts-schema-validate._

## Per-task

| Task | Runs | Model | Solve rate |
|---|---|---|---|
| go-csv-quoting | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-csv-quoting | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-csv-quoting | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| go-lru-cache | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-lru-cache | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-lru-cache | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| go-money-allocate | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-money-allocate | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-money-allocate | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| go-parallel-map | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-parallel-map | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-parallel-map | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| go-stack-pop-bug | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-stack-pop-bug | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-stack-pop-bug | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| go-ttl-lru-cache | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-ttl-lru-cache | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-ttl-lru-cache | 3 | zai:glm-5.2 | 2/3 (0.6667) |
| go-worker-pool | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| go-worker-pool | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| go-worker-pool | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-go-errwrap-unwrap | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-go-errwrap-unwrap | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-go-errwrap-unwrap | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-go-m2-01 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-go-m2-01 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-go-m2-01 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-go-m3-04 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-go-m3-04 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-go-m3-04 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-go-metrics-labels | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-go-metrics-labels | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-go-metrics-labels | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-inflection-titleize | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-inflection-titleize | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-inflection-titleize | 3 | zai:glm-5.2 | 1/3 (0.3333) |
| oss-pilot-acme-retry | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-pilot-acme-retry | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-pilot-acme-retry | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-py-config-merge | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-py-config-merge | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-py-config-merge | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-py-ledger-rounding | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-py-ledger-rounding | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-py-ledger-rounding | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-py-m2-00 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-py-m2-00 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-py-m2-00 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-py-m3-00 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-py-m3-00 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-py-m3-00 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-ts-router-params | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-ts-router-params | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-ts-router-params | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-ts-validate-coerce | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-ts-validate-coerce | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-ts-validate-coerce | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-ty-m2-02 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-ty-m2-02 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-ty-m2-02 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| oss-ty-m3-05 | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| oss-ty-m3-05 | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| oss-ty-m3-05 | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-csv-export-feature | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-csv-export-feature | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-csv-export-feature | 3 | zai:glm-5.2 | 2/3 (0.6667) |
| py-expr-eval | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-expr-eval | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-expr-eval | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-jsonpointer | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-jsonpointer | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-jsonpointer | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-retry-refactor | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-retry-refactor | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-retry-refactor | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-sliding-window-max | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-sliding-window-max | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-sliding-window-max | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-topo-sort-cycle | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-topo-sort-cycle | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-topo-sort-cycle | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| py-ttl-cache-expiry | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| py-ttl-cache-expiry | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| py-ttl-cache-expiry | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| rs-borrow-split | 3 | anthropic:claude-opus-4-8 | 0/3 (0.0000) |
| rs-borrow-split | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| rs-borrow-split | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| rs-feature-gate | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| rs-feature-gate | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| rs-feature-gate | 3 | zai:glm-5.2 | 3/3 (1.0000) |
| ts-debounce | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| ts-debounce | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| ts-debounce | 3 | zai:glm-5.2 | 0/3 (0.0000) |
| ts-deep-merge | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| ts-deep-merge | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| ts-deep-merge | 3 | zai:glm-5.2 | 2/3 (0.6667) |
| ts-event-emitter | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| ts-event-emitter | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| ts-event-emitter | 3 | zai:glm-5.2 | 2/3 (0.6667) |
| ts-querystring-bug | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| ts-querystring-bug | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| ts-querystring-bug | 3 | zai:glm-5.2 | 2/3 (0.6667) |
| ts-schema-validate | 3 | anthropic:claude-opus-4-8 | 3/3 (1.0000) |
| ts-schema-validate | 3 | openai:gpt-5.5 | 3/3 (1.0000) |
| ts-schema-validate | 3 | zai:glm-5.2 | 3/3 (1.0000) |

## Environment

- Models: anthropic:claude-opus-4-8, openai:gpt-5.5, zai:glm-5.2
- Python: 3.14.3, Python 3.12.13
- bandit: bandit 1.9.4
- git: git version 2.39.5
- go: go version go1.23.4 linux/amd64
- node: v22.11.0
- radon: 6.0.1
- ruff: ruff 0.15.18

## Calibration

> ⚠️ **20 task(s) have empirical difficulty that disagrees with the hand label**: go-csv-quoting, go-lru-cache, go-parallel-map, go-ttl-lru-cache, go-worker-pool, oss-go-errwrap-unwrap, oss-go-metrics-labels, oss-pilot-acme-retry, oss-py-config-merge, oss-py-ledger-rounding, oss-ts-router-params, oss-ts-validate-coerce, py-expr-eval, py-jsonpointer, py-retry-refactor, py-sliding-window-max, py-topo-sort-cycle, rs-feature-gate, ts-deep-merge, ts-event-emitter

Thresholds: easy ≥ 0.85, medium ≥ 0.4. Criteria: ≥ 5 attempts, ≥ 2 models. Excluded: 0 mock, 0 stale, 0 unknown-task runs.

| Task | Label | Empirical | Solve rate ± se | Attempts | Models |
|---|---|---|---|---|---|
| go-csv-quoting | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| go-lru-cache | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| go-parallel-map | hard | easy | 1.0000 ± 0.0000 | 9 | 3 |
| go-ttl-lru-cache | hard | easy | 0.8889 ± 0.1111 | 9 | 3 |
| go-worker-pool | hard | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-go-errwrap-unwrap | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-go-metrics-labels | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-pilot-acme-retry | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-py-config-merge | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-py-ledger-rounding | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-ts-router-params | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-ts-validate-coerce | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-expr-eval | hard | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-jsonpointer | hard | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-retry-refactor | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-sliding-window-max | hard | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-topo-sort-cycle | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| rs-feature-gate | medium | easy | 1.0000 ± 0.0000 | 9 | 3 |
| ts-deep-merge | hard | easy | 0.8889 ± 0.1111 | 9 | 3 |
| ts-event-emitter | medium | easy | 0.8889 ± 0.1111 | 9 | 3 |
| go-money-allocate | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| go-stack-pop-bug | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-go-m2-01 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-go-m3-04 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-inflection-titleize | medium | medium | 0.7778 ± 0.2222 | 9 | 3 |
| oss-py-m2-00 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-py-m3-00 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-ty-m2-02 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| oss-ty-m3-05 | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| py-csv-export-feature | easy | easy | 0.8889 ± 0.1111 | 9 | 3 |
| py-ttl-cache-expiry | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |
| rs-borrow-split | medium | medium | 0.6667 ± 0.3333 | 9 | 3 |
| ts-debounce | medium | medium | 0.6667 ± 0.3333 | 9 | 3 |
| ts-querystring-bug | easy | easy | 0.8889 ± 0.1111 | 9 | 3 |
| ts-schema-validate | easy | easy | 1.0000 ± 0.0000 | 9 | 3 |

4 task(s) lack sufficient data (5+ attempts from 2+ models).

