# Router eval scripts

Tools for the VulcanBench Router v1 study (`tasks/router-v1`, design and
budget in `docs/DECISIONS.md`, 2026-11-05). Two questions: does a model
router change the served model with the task cell (language x repo size x
difficulty), and what does a router cost per task against a direct API at
list price and against a flat subscription seat.

Three scripts, all standard library plus matplotlib for the cards. Nothing
here changes harness behaviour; the harness side (served model and router
cost per step, usage-window snapshots for Claude Code runs) is in
`harness/agent/routing.py` and `harness/agent/quota_probe.py`.

## 1. `probe_ladder.py`: single-request routing probe

One chat request per task per repeat, no tools: the issue, a file tree, and
the files the gold patch touches, trimmed to a token budget per repo scale
(default 6k for small, 24k for large; tokens are approximated as characters
divided by four). The response's `model` is the served model and
`usage.cost`, when the gateway returns one, is what it charged.

A **length control** repeats each small+easy prompt padded with files from
the large+easy task of the same language up to the large budget, labelled
`control: "padded"`. If the served-model mix moves with padding but not with
the cell, the router routes on length, not content.

```bash
python scripts/router-eval/probe_ladder.py --dry-run           # sizes only
python scripts/router-eval/probe_ladder.py \
  --router openrouter=https://openrouter.ai/api/v1=openrouter/auto \
  --router martian=https://api.withmartian.com/v1=auto \
  --n 8 --out ladder.jsonl
```

Routers are `name=BASE_URL=MODEL`; `name=MODEL` reads
`ROUTER_<NAME>_BASE_URL`. Keys: `OPENROUTER_API_KEY` for `openrouter`,
otherwise `ROUTER_<NAME>_API_KEY`. Rows already in `--out` are skipped, so a
stopped run resumes. The payload carries `usage: {include: true}`.

One JSONL row per request:

| field | meaning |
| --- | --- |
| `router`, `requested_model` | the router and the model id sent |
| `task_id`, `cell`, `control`, `rep` | task, its `router_cell`, `"padded"` or null, repeat index |
| `budget_tokens`, `approx_prompt_tokens` | budget and the prompt's estimated size |
| `served_model`, `response_id`, `provider` | what the response body reported |
| `prompt_tokens`, `completion_tokens`, `reported_cost_usd` | from the response `usage` (`cost` may be absent) |
| `finish_reason`, `latency_s`, `answer_chars` | response shape |
| `error` | the HTTP or network error, when the request failed |

## 2. `analyze.py`: routing sensitivity and cost

```bash
python scripts/router-eval/analyze.py \
  --ladder ladder.jsonl \
  --runs runs-router-openrouter --leg openrouter \
  --runs runs-router-martian   --leg martian \
  --runs runs-claude-max       --leg claude-max \
  --plan-fee claude-max=200 \
  --manual-capacity cursor=35 \
  --out analysis.json --csv-dir csv/
```

Run folders are `<task>-<hex>/summary.json` as `vulcanbench run` writes
them; `--runs` and `--leg` are given in pairs. `--plan-fee` is the monthly
seat price of a subscription leg. `--manual-capacity` is the list-price
dollars one week of a seat buys, read by hand from a product's usage page
for legs whose stream carries no usage window (Cursor, Codex).

`analysis.json`:

- `routing.ladder[router]` and `routing.runs[leg]`: served-model share per
  cell (`by_cell`, with `n`), the chi-square test of cell vs served model
  and the normalised mutual information (`cell_test`), the same per factor
  (`by_factor.language|scale|difficulty`), the padded-vs-plain
  `length_control` (ladder), share by prompt-size quartile
  (`by_prompt_quartile`, ladder), `within_run_switch_rate` (runs: served
  model changes per step), error counts, mean reported cost and latency.
- `cost[leg]`: per leg and per cell, `cash` and `api_equivalent` blocks with
  n, mean, standard error, cost per solved task and pass rate, plus mean
  minutes and tokens, and the `cost_basis` mix. Subscription legs have
  `cash.mean_usd` null: the seat is the cash, never a per-task zero.
- `subscription_capacity[leg]`: for legs whose runs carry
  `economics.quota.delta`, the pooled list-price cost divided by the pooled
  seven-day window delta gives the list-price dollars one week of the seat
  buys (`capacity_usd_week`), then per month, the effective discount against
  list, the tasks per month one seat covers at the leg's mean cost per task,
  and the seat's effective dollars per task at capacity.
- `break_even`: monthly cost per engineer against tasks per month for every
  leg (subscription legs add a seat when demand passes one seat's capacity),
  and per subscription/metered pair the demand below which the metered leg
  is cheaper than a seat.

Statistics are plain Python: the chi-square p-value comes from the
regularised upper incomplete gamma function (series below s+1, Lentz
continued fraction above), and NMI is I(X;Y)/sqrt(H(X)H(Y)). Cells with ten
or so observations make the chi-square approximate; read it next to the
share table, not alone.

CSVs: `routing_shares.csv`, `routing_tests.csv`, `cost_by_leg.csv`,
`breakeven.csv`.

## 3. `make_router_cards.py`: brand-styled PNGs

```bash
python scripts/router-eval/make_router_cards.py analysis.json --out-dir docs/results/router-v1-2026-11
```

`router_routing_card.png` stacks the served-model share per cell for each
router (ladder) and each agent leg that reported served models, with n per
column and the test results in the panel title. `router_cost_card.png`
shows cost per task per leg with n, pass rate and one standard error, and
the monthly cost-per-engineer curves with crossovers. Fonts and the logo
chip come from `scripts/rankings-chart/`; subscription legs are drawn as
API-equivalent and labelled so; list-price caveats are footnoted; there are
no em-dashes in rendered text.

## Tests

`tests/test_router_eval.py` covers the chi-square and NMI maths on
hand-built tables, the capacity and break-even formulas on synthetic
summaries, the length-control contrast, the prompt builder on a suite task,
and that both cards render.
