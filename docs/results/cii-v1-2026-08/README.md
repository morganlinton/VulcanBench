# CII v1 results (August 2026): withdrawn

The VulcanBench Coding Intelligence Index v1 and the results that were
published here are withdrawn as of 2026-10-06.

An audit found that six CII v1 tasks shipped compiled copies of their hidden
tests in the agent's starting workspace (Python bytecode left behind by
running the tests in place), and that agents decoded those files on at least
one task in the published set. The published numbers cannot be relied on, and
the suite was retired rather than re-run. The cause is fixed for every suite:
local bytecode is no longer copied into agent workspaces.

The full finding is in [docs/DECISIONS.md](../../DECISIONS.md) (2026-10-06).
The suite, the chart and the original report remain in git history.

For current results, see VulcanBench Frontier v4
([tasks/coding-intelligence-index-v4](../../../tasks/coding-intelligence-index-v4/)).
