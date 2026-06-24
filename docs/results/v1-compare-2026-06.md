# VulcanBench report — v1

_Generated 2026-06-24T18:12:15.773823+00:00_

- **156** runs · **3** models · **52** tasks · total cost $21.690443

> ⚠️ **3 run(s) scored against non-decontaminated task(s)** (oss-inflection-titleize) — these tasks derive from public sources that predate model training cutoffs; treat their scores with care.

## Models

| Model | Tasks | Runs | pass@1 ± se | pass@k | Avg total | Cost $ | Avg time |
|---|---|---|---|---|---|---|---|
| openai:gpt-5.5 | 52 | 52 | 0.7885 ± 0.0572 | 0.7885 | 0.8332 | 6.2324 | 58.4245 |
| anthropic:claude-opus-4-8 | 52 | 52 | 0.7885 ± 0.0572 | 0.7885 | 0.8280 | 6.2664 | 46.3653 |
| zai:glm-5.2 | 52 | 52 | 0.7115 ± 0.0634 | 0.7115 | 0.7082 | 9.1917 | 191.3339 |

## Per-task

| Task | Runs | Model | Solve rate |
|---|---|---|---|
| go-csv-quoting | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| go-csv-quoting | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| go-csv-quoting | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| go-lru-cache | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| go-lru-cache | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| go-lru-cache | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| go-money-allocate | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| go-money-allocate | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| go-money-allocate | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| go-stack-pop-bug | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| go-stack-pop-bug | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| go-stack-pop-bug | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| go-worker-pool | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| go-worker-pool | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| go-worker-pool | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-errwrap-unwrap | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-errwrap-unwrap | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-errwrap-unwrap | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m2-01 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m2-01 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m2-01 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m2-04 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m2-04 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m2-04 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m2-07 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m2-07 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m2-07 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m3-01 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m3-01 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m3-01 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m3-04 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m3-04 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m3-04 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m3-07 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m3-07 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m3-07 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m3-10 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m3-10 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m3-10 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-m3-13 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-m3-13 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-m3-13 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-go-metrics-labels | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-go-metrics-labels | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-go-metrics-labels | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-inflection-titleize | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-inflection-titleize | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-inflection-titleize | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-pilot-acme-retry | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-pilot-acme-retry | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-pilot-acme-retry | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-py-cache-evict | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-cache-evict | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-cache-evict | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-config-merge | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-py-config-merge | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-py-config-merge | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-py-ledger-rounding | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-py-ledger-rounding | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-py-ledger-rounding | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m2-00 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m2-00 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m2-00 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m2-03 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m2-03 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m2-03 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-py-m2-06 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m2-06 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m2-06 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m2-09 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m2-09 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m2-09 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-00 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-00 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-00 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-03 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-03 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-03 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-06 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-06 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-06 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-09 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-09 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-09 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-12 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-12 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-12 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-py-m3-15 | 1 | anthropic:claude-opus-4-8 | 0/1 (0.0000) |
| oss-py-m3-15 | 1 | openai:gpt-5.5 | 0/1 (0.0000) |
| oss-py-m3-15 | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| oss-ts-router-params | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ts-router-params | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ts-router-params | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ts-validate-coerce | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ts-validate-coerce | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ts-validate-coerce | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m2-02 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m2-02 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m2-02 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m2-05 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m2-05 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m2-05 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m2-08 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m2-08 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m2-08 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m3-02 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m3-02 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m3-02 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m3-05 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m3-05 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m3-05 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m3-08 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m3-08 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m3-08 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m3-11 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m3-11 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m3-11 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| oss-ty-m3-14 | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| oss-ty-m3-14 | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| oss-ty-m3-14 | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| py-csv-export-feature | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| py-csv-export-feature | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| py-csv-export-feature | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| py-jsonpointer | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| py-jsonpointer | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| py-jsonpointer | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| py-retry-refactor | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| py-retry-refactor | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| py-retry-refactor | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| py-topo-sort-cycle | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| py-topo-sort-cycle | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| py-topo-sort-cycle | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| py-ttl-cache-expiry | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| py-ttl-cache-expiry | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| py-ttl-cache-expiry | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| rs-borrow-split | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| rs-borrow-split | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| rs-borrow-split | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| rs-feature-gate | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| rs-feature-gate | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| rs-feature-gate | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| ts-debounce | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| ts-debounce | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| ts-debounce | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| ts-deep-merge | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| ts-deep-merge | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| ts-deep-merge | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| ts-event-emitter | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| ts-event-emitter | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| ts-event-emitter | 1 | zai:glm-5.2 | 0/1 (0.0000) |
| ts-querystring-bug | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| ts-querystring-bug | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| ts-querystring-bug | 1 | zai:glm-5.2 | 1/1 (1.0000) |
| ts-schema-validate | 1 | anthropic:claude-opus-4-8 | 1/1 (1.0000) |
| ts-schema-validate | 1 | openai:gpt-5.5 | 1/1 (1.0000) |
| ts-schema-validate | 1 | zai:glm-5.2 | 1/1 (1.0000) |

## Environment

- Models: anthropic:claude-opus-4-8, openai:gpt-5.5, zai:glm-5.2
- Python: 3.14.3, Python 3.12.13
- bandit: bandit 1.9.4
- git: git version 2.39.5
- go: go version go1.23.4 linux/amd64
- node: v22.11.0
- radon: 6.0.1
- ruff: ruff 0.15.18
