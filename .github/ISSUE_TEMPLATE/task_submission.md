---
name: Task submission
about: Propose a new VulcanBench benchmark task
title: "Task submission: <task-id>"
labels: task-submission
---

<!-- The dashboard /submit page pre-fills this for you. See docs/TASK_CONTRIBUTION.md. -->

**Proposed task id:** `<task-id>`
**Category:** <bug_fix | feature | refactor>
**Language(s):** <python | go | typescript | javascript>

**Issue (what the agent is asked to do — no solution):**

<describe the bug or feature>

---

### Checklist (see `docs/TASK_CONTRIBUTION.md`)

- [ ] `repo/` starting state + hidden `tests/` + `gold_patch.diff`
- [ ] declarative `fail_to_pass` / `pass_to_pass` in `metadata.json`
- [ ] honest `source` + `decontamination_notes` (and provenance if `source: oss`)
- [ ] `python scripts/validate_tasks.py tasks/v1/<task-id>` passes
      (gold solves it, fail-to-pass genuinely fails pre-patch, deterministic)
