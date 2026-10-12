<!-- BEGIN:nextjs-agent-rules -->
# This is NOT the Next.js you know

This version has breaking changes: APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` before writing any code. Heed deprecation notices.
<!-- END:nextjs-agent-rules -->

# Writing conventions (applies to all written text)

Never use em-dashes ("—", U+2014) in written text: docs, commit messages, PR
descriptions, chat replies, or UI copy the dashboard renders for readers. Code
itself (source, comments, tests, config) is exempt. In written text, restructure
instead: use a comma, a colon, parentheses, or two sentences. A
plain hyphen ("-") is fine for ranges and compound words. Avoid en-dashes ("–")
too; write "to" for ranges. See the repo root CLAUDE.md / AGENTS.md for the
canonical rule.
