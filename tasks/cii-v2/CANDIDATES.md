# cii-v2 candidate log

Every candidate that reaches the frontier gate gets a row here — admits and
rejects alike. Solve = functional 1.0. Bar: best model ≤1/3.

| candidate | correctness gate | codex:gpt-5.6-sol (3 runs) | claude-code:claude-opus-5 (3 runs) | verdict |
|---|---|---|---|---|
| oss-networkx-kcomponents-exact-epic | PASS (gold=1.0, pre=0.0, det ×3) | 1.0, 1.0, 1.0 (3/3) | 1.0, 1.0, 1.0 (3/3) | **REJECT** — solved 6/6; recycle to v1 candidate pool |
| oss-zod-optionality-ladder | PASS (gold=1.0, pre=0.0, det ×3) | 1.0, 1.0, 0.0 (2/3) | 1.0, 1.0, 1.0 (3/3) | **REJECT** (re-gated under ≤2/3 hard band: Opus 3/3) — recycled to v1 |
| oss-zod-record-intersection-strictness | PASS (det ×3) | 1.0, 0.75, 0.75 (1/3) | 1.0, 1.0, 1.0 (3/3) | **REJECT** (hard band) — but codex 1/3 makes it a strong v1 discriminator; recycled |
| oss-undici-cache-revalidation-epic | PASS (det ×3) | 1.0, 1.0, 0.8 (2/3) | 1.0, 1.0, 1.0 (3/3) | **REJECT** (hard band) — recycled to v1 |
| oss-sqlglot-json-operator-precedence | PASS (det ×3) | 1.0, 1.0, 1.0 (3/3, early exit) | not run | **REJECT** — recycled to v1 |

**Wave 1 conclusion (5 candidates, 0 admits, both bars):** Opus 5 solved every gated run — 15/15 across four candidates (sqlglot skipped by early exit). GPT 5.6 Sol missed runs on three of five (record-intersection 1/3, ladder 2/3, undici 2/3), so the format still discriminates BETWEEN frontier models — it just cannot beat the stronger one. The binding constraint is Opus 5, not the bar.
