# Rankings chart pipeline

Generates the shareable three-panel suite-v3 results PNG (pass@1 rankings,
speed panel, per-model effort-curve cards) in VulcanBench branding.

## Usage

```bash
pip install matplotlib   # numpy + pillow come with it
python scripts/rankings-chart/update_rankings.py   # aggregate ./runs data
python scripts/rankings-chart/make_chart.py        # render the PNG
```

Output: `vulcanbench_suite3_rankings.png` (2560×3360) next to the scripts.

## How aggregation works

- Non-repeat models: best `suite.json` aggregate per (model, effort).
- Repeat-swept models (`REPEAT_MODELS` in `update_rankings.py`): pass@1 is the
  mean of per-task success rates across ALL fresh runs; stderr is the std of
  per-task means / sqrt(n_tasks). Add a model to that set after giving it
  `--repeat 3` sweeps.
- DeepSeek runs requested at `medium` are excluded (its API silently coerces
  `medium` to `high`; see CHANGELOG for the effort-mapping fix).
- Claude Opus 5 rows are hardcoded from vulcanbench.com Report 10 (those runs
  aren't in this repo); remove the block once local Opus 5 runs exist.

## Assets & licenses

- **Fonts**: Geist (Vercel) and Chakra Petch, both from Google Fonts under the
  SIL Open Font License 1.1 — https://openfontlicense.org. Chakra Petch static
  weights register in matplotlib as separate families ("Chakra Petch",
  "… Medium", "… SemiBold").
- **Lab logos** (`logos/`): white silhouettes derived from each company's mark
  (simple-icons for Anthropic/DeepSeek/Moonshot/Qwen; Wikimedia Commons for
  OpenAI and xAI). Used nominatively to identify the systems under test — see the
  trademark note in the root README. All marks belong to their owners.
- **VulcanBench logo**: `vb_logo_rounded.png`, derived from
  https://vulcanbench.com/assets/logo.png (see also `docs/assets/`).
