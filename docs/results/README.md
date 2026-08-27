# Published benchmark snapshots

Point-in-time results from full suite runs. These files are **generated**: do not
edit by hand. Reproduce from local `./runs` with:

```bash
# Markdown + JSON (machine-readable source of truth for the PDF)
vulcanbench report --suite v1 -o docs/results/v1-compare-2026-06.md
vulcanbench report --suite v1 -f json -o docs/results/v1-compare-2026-06.json

# PDF summary (requires fpdf2: pip install fpdf2)
python scripts/export_results_pdf.py docs/results/v1-compare-2026-06.json \
  -o docs/results/v1-compare-2026-06.pdf
```

## Naming

`<suite>-<slug>-<YYYY-MM>.{md,json,pdf}`: e.g. `v1-compare-2026-06` for the June 2026
three-model v1 comparison (GLM 5.2, Opus 4.8, GPT 5.5).

Raw run artifacts stay in `./runs/` (gitignored). Commit only the exported summaries here
when you want a permanent link in the repo or release notes.

## Reports

Numbered technical reports, newest first. Each dated directory holds its
`model-card.md` (the narrative), a machine-readable JSON, and a branded chart.

| No. | Report | Directory |
|---|---|---|
| 20 | Muse Spark 1.2: model versus harness (raw API vs the Pi harness), Harness Study No. 04 | [v3-musespark-harness-2026-08](v3-musespark-harness-2026-08/model-card.md) |
| 19 | Muse Spark 1.2 across the effort knob | [v3-musespark-2026-08](v3-musespark-2026-08/model-card.md) |
| 18 | GLM 5.3: model versus harness (raw API vs the ZCode harness), Harness Study No. 03 | [v3-glm53-2026-08](v3-glm53-2026-08/model-card.md) |
| 12 | Qwen3.8-Max across the effort knob | [v3-qwen38-max-2026-08](v3-qwen38-max-2026-08/model-card.md) |
| 11 | Grok Voice Think Fast 2.0 vs GPT Realtime | [voice-v1-2026-07](voice-v1-2026-07/model-card.md) |
| 10 | Claude Opus 5 across the effort knob | [v3-opus5-effort-2026-07](v3-opus5-effort-2026-07/model-card.md) |
| 09 | Does training-data contamination move Claude Opus 5's score? | [v4-contamination-2026-07](v4-contamination-2026-07/model-card.md) |
| 04 | Grok 4.5 vs Claude Fable 5 vs GPT-5.6 Sol | [v3-3way-2026-07](v3-3way-2026-07/model-card.md) |

Reports 13 to 17 (DeepSeek V4, Grok 4.6, two Grok 4.6 harness studies, and
Qwen3.8-27B) are published on vulcanbench.com/benchmarks and are not mirrored in
this checkout, which is why the table jumps from 12 to 18.

Report No. 18 (Harness Study No. 03) measures one model (GLM 5.3) through two
harnesses on the identical v3 suite: VulcanBench's uniform loop on the raw `zai`
API, and Z.ai's own ZCode harness on a GLM Coding Plan. Its `subscription` track
must never be added to a raw-API leaderboard; see the model card's caveats.

Report No. 20 (Harness Study No. 04) is the same design for Muse Spark 1.2:
uniform loop (Report No. 19) versus the Pi harness on metered API. `pi:` rows
must never be added as a second raw-API board entry; from v0.9.1 the CLI
enforces this (`leaderboard --track api` filters them out). See that model card.
