# VulcanBench agent instructions

Claude Code reads CLAUDE.md; other agents (Cursor, Codex, Grok, and similar)
read this file. The project notes, brand, and chart-integrity rules live in
[CLAUDE.md](CLAUDE.md); read it as well. Run-condition decisions (timeouts,
concurrency) and their evidence are in [docs/DECISIONS.md](docs/DECISIONS.md). How suite
building and benchmark runs stay apart (separate PRs, frozen suites, tagged run
worktrees) is in [docs/HOW_WE_WORK.md](docs/HOW_WE_WORK.md). The rule below is the one that applies
to every agent and all written text.

## Writing conventions (applies to all written text)

Never use em-dashes ("—", U+2014) in written text: docs and Markdown, commit
messages, PR descriptions, chat replies, reports, charts, social images, or any
other prose a person reads. Text that code renders for readers (chart titles and
labels, card copy, report bodies, UI copy) is written text too.

Code is exempt. Em-dashes are allowed in source files, including comments,
docstrings, string literals, tests and config. The CI check
(`scripts/check_no_emdash.py`) scans prose files only (.md, .mdx, .mdc, .txt).

When you would reach for an em-dash in written text, restructure instead. Use
a comma, a colon, a pair of parentheses, or split into two sentences. A plain hyphen ("-") is fine
for ranges and compound words. En-dashes ("–", U+2013) should also be avoided;
write "to" for ranges (for example "10 to 20", not "10–20").

Keep headings plain (no decorative dashes around them) on shareable assets.
