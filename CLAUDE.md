# VulcanBench project notes

## Writing conventions (applies to ALL output)

Never use em-dashes ("—", U+2014) anywhere: not in code, comments, commit
messages, docs, chat replies, charts, reports, social images, or any other
generated text. This rule has no exceptions.

When you would reach for an em-dash, restructure instead. Use a comma, a colon,
a pair of parentheses, or split into two sentences. A plain hyphen ("-") is fine
for ranges and compound words. En-dashes ("–", U+2013) should also be avoided;
use "to" for ranges (for example "10 to 20", not "10–20").

Keep headings plain (no decorative dashes around them) on shareable assets.

## Suite names

The public name of `tasks/coding-intelligence-index-v4` is **VulcanBench
Frontier v4** (renamed from VulcanBench-SWE v4 on 2026-09-18; URL slugs,
file names and run directories keep `swe-v4` and are never renamed). Its
private routine companion is VulcanBench Routine v1, and the private conduct
suite in the VulcanConduct repo (suite id `conduct-v1`) is VulcanBench Safety v1
(renamed from VulcanConduct v1 on 2026-09-18). Use the public names in
anything user-facing; never write "SWE v4" for new material.

## Operating decisions

Run-condition decisions (task timeouts, sweep concurrency, and why) are
logged with their evidence in [docs/DECISIONS.md](docs/DECISIONS.md). Read
the relevant entry before changing budgets, concurrency, or sweep launchers,
and add an entry when a decision like that is made. Current: v4 tasks carry
a flat 3-hour timeout; sweeps run one task at a time.

Operator settings live in [vulcanbench.toml](vulcanbench.toml). Effort levels
listed under `[effort].blocked` (currently "ultra") never run on any model,
harness or suite; the harness refuses them before a model call. Do not work
around the block or add an ultra column anywhere.

## Brand: logo and typography

Use these whenever producing anything user-facing or shareable (charts, reports,
social images, docs headers) for VulcanBench.

- **Logo**: [docs/assets/vulcanbench-logo.png](docs/assets/vulcanbench-logo.png)
  is the canonical mark: a white angular layered "V" on a black square
  (1078×1078 PNG, no alpha). Live copy at https://vulcanbench.com/assets/logo.png.
  Present it as a rounded-corner chip (~22% corner radius) next to the wordmark.
  ⚠️ The dashboard favicon (`dashboard/app/favicon.ico`, black circle + white
  triangle) is an outdated placeholder. Do NOT use it as the logo.
- **Wordmark**: "VulcanBench" set in **Chakra Petch SemiBold (600)**, black
  (`#0b0b0b`) on light backgrounds, white on dark. Google Fonts family
  "Chakra Petch"; static per-weight TTFs register as separate families
  ("Chakra Petch", "Chakra Petch Medium", "Chakra Petch SemiBold") in
  matplotlib: address them by those names.
- **Headings / display text**: Chakra Petch (Medium for section titles).
- **Secondary / code face**: IBM Plex Mono (400/500), vulcanbench.com's
  second family.
- **Brand palette**: monochrome black/white. The dashboard app UI accent is
  emerald-500 (`#10b981`), and the dashboard app font is Geist (via
  `next/font`). Those are app-UI choices, not the marketing brand; the
  wordmark face is always Chakra Petch.

## Shareable results charts

The generator for the three-panel suite-v3 results PNG (pass@1 rankings,
speed panel, effort-curve cards, all in brand styling) lives in
[scripts/rankings-chart/](scripts/rankings-chart/). See its README for
usage, aggregation rules, and asset licenses. Per-lab colors: Anthropic clay
`#D97757`, OpenAI `#10A37F`, DeepSeek `#5786FE`, xAI black `#0A0A0A`,
Moonshot dark slate `#44445E` (official Moonshot black collides with xAI, so
slate is the deliberate stand-in). Chart-integrity rules that must survive
edits: per-column run counts (`n=`) and ±1 stderr whiskers stay visible;
partial-coverage columns keep their asterisks; externally sourced columns
(e.g. Opus 5 from Report 10) and list-price cost caveats stay footnoted.
