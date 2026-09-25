# VulcanBench agent instructions

Claude Code reads CLAUDE.md; other agents (Cursor, Codex, Grok, and similar)
read this file. The project notes, brand, and chart-integrity rules live in
[CLAUDE.md](CLAUDE.md); read it as well. Run-condition decisions (timeouts,
concurrency) and their evidence are in [docs/DECISIONS.md](docs/DECISIONS.md). The rule below is the one that applies
to every agent and every kind of output.

## Writing conventions (applies to ALL output)

Never use em-dashes ("—", U+2014) anywhere: not in code, comments, commit
messages, docs, chat replies, charts, reports, social images, or any other
generated text. This rule has no exceptions.

When you would reach for an em-dash, restructure instead. Use a comma, a colon,
a pair of parentheses, or split into two sentences. A plain hyphen ("-") is fine
for ranges and compound words. En-dashes ("–", U+2013) should also be avoided;
write "to" for ranges (for example "10 to 20", not "10–20").

Keep headings plain (no decorative dashes around them) on shareable assets.
