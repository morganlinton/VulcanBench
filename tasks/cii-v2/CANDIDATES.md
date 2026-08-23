# cii-v2 candidate log

Every candidate that reaches the frontier gate gets a row here — admits and
rejects alike. Solve = functional 1.0. Bar: best model ≤1/3.

| candidate | correctness gate | codex:gpt-5.6-sol (3 runs) | claude-code:claude-opus-5 (3 runs) | verdict |
|---|---|---|---|---|
| oss-networkx-kcomponents-exact-epic | PASS (gold=1.0, pre=0.0, det ×3) | 1.0, 1.0, 1.0 (3/3) | 1.0, 1.0, 1.0 (3/3) | **REJECT** — solved 6/6; recycle to v1 candidate pool |
| oss-zod-optionality-ladder | PASS (gold=1.0, pre=0.0, det ×3) | 1.0, 1.0, 0.0 (2/3) | not run (early reject) | **REJECT** — codex 2/3 already over the bar; Opus runs skipped. Closest miss yet; recycle to v1 candidate pool |
