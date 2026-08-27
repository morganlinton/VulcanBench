# Rankings chart pipeline

Generates the shareable four-panel suite-v3 results PNG (pass@1 rankings,
speed, average API cost per task run, and per-model effort-curve cards) in
VulcanBench branding. The three bar panels show one column per model at its
best-scoring effort level, ties breaking to the cheaper run (`best_effort_row`
in `make_chart.py`); the effort-curve cards still plot every effort level.

## Usage

```bash
pip install matplotlib   # numpy + pillow come with it
python scripts/rankings-chart/update_rankings.py   # aggregate ./runs data
python scripts/rankings-chart/make_chart.py        # render the PNG
```

Output: `vulcanbench_suite3_rankings.png` (2560×4800) next to the scripts.

For sharing on X, use the single-metric 16:9 cards instead; the tall composite
gets scaled down until its labels are unreadable:

```bash
python scripts/rankings-chart/make_cards.py     # rankings, speed, cost cards
python scripts/rankings-chart/make_efforts.py   # effort-curve card
python scripts/rankings-chart/make_musespark_harness.py    # Report 20 two-panel
python scripts/rankings-chart/make_musespark_studycard.py  # Report 20 study card
python scripts/rankings-chart/make_musespark_hero.py       # Report 20 16:9 hero
```

These emit `vulcanbench_suite3_card_{rankings,speed,cost}.png` and
`vulcanbench_suite3_efforts.png` (the bar cards are 2560×2080, the efforts card 2560×1440; all with large horizontal labels),
which together fit X's four-image limit. The three bar cards use the same
best-scoring-effort-per-model rule as the composite (`top_per_model(rows,
rule="best")` in `_common.py`; `rule="max"` picks the highest tested effort
instead); `make_efforts.py` keeps every effort level.

## How aggregation works

- Non-repeat models: best `suite.json` aggregate per (model, effort).
- Repeat-swept models (`REPEAT_MODELS` in `update_rankings.py`): pass@1 is the
  mean of per-task success rates across all runs whose task hashes match the
  current frozen suite; stderr is the std of per-task means / sqrt(n_tasks).
  Add a model to that set after giving it `--repeat 3` sweeps.
- DeepSeek runs requested at `medium` are excluded (its API silently coerces
  `medium` to `high`; see CHANGELOG for the effort-mapping fix).
- Claude Opus 5 rows are hardcoded from vulcanbench.com Report 10 (those runs
  aren't in this repo); remove the block once local Opus 5 runs exist.

## Assets & licenses

- **Fonts**: Geist (Vercel) and Chakra Petch, both from Google Fonts under the
  SIL Open Font License 1.1, https://openfontlicense.org. Chakra Petch static
  weights register in matplotlib as separate families ("Chakra Petch",
  "… Medium", "… SemiBold").
- **Lab logos** (`logos/`): white silhouettes derived from each company's mark
  (simple-icons for Anthropic/DeepSeek/Moonshot/Qwen; Wikimedia Commons for
  OpenAI and xAI). Used nominatively to identify the systems under test, see the
  trademark note in the root README. All marks belong to their owners.
- **VulcanBench logo**: `vb_logo_rounded.png`, derived from
  https://vulcanbench.com/assets/logo.png (see also `docs/assets/`).
