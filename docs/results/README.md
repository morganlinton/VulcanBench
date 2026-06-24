# Published benchmark snapshots

Point-in-time results from full suite runs. These files are **generated** — do not
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

`<suite>-<slug>-<YYYY-MM>.{md,json,pdf}` — e.g. `v1-compare-2026-06` for the June 2026
three-model v1 comparison (GLM 5.2, Opus 4.8, GPT 5.5).

Raw run artifacts stay in `./runs/` (gitignored). Commit only the exported summaries here
when you want a permanent link in the repo or release notes.
