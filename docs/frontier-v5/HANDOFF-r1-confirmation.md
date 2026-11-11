# Frontier v5 release 1 confirmation: handoff (2026-11-10)

State of the VulcanBench Frontier v5 release 1 confirmation runs as of
2026-11-10, written when the owner moved this work from Claude Code to Codex.
Read [AGENTS.md](../../AGENTS.md) and [CLAUDE.md](../../CLAUDE.md) first (the
project rules apply to every agent), then [HOW_WE_WORK.md](../HOW_WE_WORK.md),
the [DECISIONS.md](../DECISIONS.md) entries dated 2026-11-02 and 2026-10-31,
and [PLAN-v4.md](PLAN-v4.md) section 4.

## The rules that matter most here

- No em-dashes or en-dashes in any prose (docs, commits, PR text, chat).
  Use the public suite names: VulcanBench Frontier v5, VulcanBench Frontier
  v4, VulcanBench Safety v1.
- Engine changes and published results go in separate PRs (CI enforces it).
  Scripts, tests and other docs are neutral.
- Ask the owner before any outward action: opening or merging a PR, pushing
  a tag or branch, posting anything. A merge needs the owner's explicit
  words for that merge.
- Never start a run that spends model quota (Claude or ChatGPT) without the
  owner's go-ahead in chat for that run. Gate and confirmation runs go only
  through `scripts/frontier-v5/run_gate.sh`, never a hand-rolled `harbor run`.
- The main checkout (`~/Documents/GitHub/VulcanBench`) belongs to other work.
  Do not edit it or switch its branch. Make a git worktree off `origin/main`
  for every change.
- The repo is public. Pushing a bench tag or task branch publishes every file
  in it. Private task material (ferrycore) lives only in the private repo
  `morganlinton/vulcanbench-frontier-private`. Never commit, push or quote
  anything from a private task directory; report its per-task numbers only.

## Where things stand

- **Release 1 is frozen** (PR #258): `tasks/frontier-v5/suite.json` and
  `suite.lock.json`, seven tasks: treesitter-query-anchor-matching,
  black-fmt-skip-region-handling, prettier-comment-attachment-stability,
  javaparser-member-resolution-scope, cli11-toml-config-round-trip,
  pulldown-cmark-html-block-scanning, and ferrycore-crossing-opaque-parity
  0.2.0 (private; the public lock holds only its hash under
  `private_tasks`, private repo commit 49c72d42).
- **Decision (m)** (DECISIONS.md, 2026-11-02 entry) funds 42 confirmation
  runs: 7 tasks, 2 references (GPT-6 Astra and Claude Opus 5.5), 3 attempts
  each, headline = complete passes over 21 per reference.
- **GPT-6 Astra half: done, 0 of 21, audited.** Bench tag
  `bench/2026-11-02-frontier-v5-r1-confirmation` (27ed21e1). Every attempt was
  a real gpt-6-astra session (10 to 160 tool calls), no connected-app or web
  calls, full verifier output, tamper scans clean. Jobs:
  `~/vulcanbench-confirm/jobs/astra-*-confirm`, logs
  `~/vulcanbench-confirm/astra-*-confirm.log` and `run-astra.log`. Do not
  rerun it.
- **Claude Opus 5.5 half: not run.** The 2026-11-03 attempt is void: the
  Claude account's organization had disabled subscription access for Claude
  Code, every attempt failed before any model call, and `run_gate.sh` scored
  21 fake zeros. Evidence kept in `~/vulcanbench-confirm/jobs/INFRA-2026-11-03-*`
  and `~/vulcanbench-confirm/INFRA-2026-11-03-*.log`. Never count them.
- **The run_gate.sh fix is merged** (PR #288, 87d23f69): the Claude usage
  pre-flight now fails closed (no rate-limit event refuses unless
  `GATE_USAGE_OVERRIDE=1`; a probe that returns an error refuses even with
  it) and the infrastructure guard matches Claude Code's auth failures
  (including `oauth_org_not_allowed`). Checked against all 21 voided trials
  (all match) and every earlier gate trial (none match).
- **Ready for an Opus half, if one is ever run:** tag
  `bench/2026-11-10-frontier-v5-r1-confirmation-opus` (87d23f69, pushed); run
  worktree `~/Documents/GitHub/VulcanBench-runs/bench-2026-11-10-frontier-v5-r1-confirmation-opus`;
  ferrycore worktree `~/vulcanbench-confirm/worktree-ferrycore` moved to that
  tag (private task dir untracked, hash matches the lock); both
  `python -m harness.suite_lock check` and
  `--private-root ~/vulcanbench-confirm/worktree-ferrycore/tasks check frontier-v5`
  pass; `~/vulcanbench-confirm/bin/confirm_runs.sh` points at the new run
  worktree. On the gate path only `run_gate.sh` differs from the Astra tag
  (`tasks/frontier-v5/` and `scripts/frontier-v5/` are otherwise unchanged).
  The Claude credential for a second account
  (`~/.config/vulcanbench/claude-gate-alt.env`) was never created.

## Open decision for the owner

On 2026-11-10 the owner asked to stop spending Claude credits and continue
with GPT-6 Astra only. The Astra half is already complete, so this means
**publishing the release 1 confirmation with one reference instead of the
two that decision (m) and the lock's `pins.references` call for.** Confirm
with the owner which of these they want before writing results:

1. **Astra-only confirmation now.** Publish the Astra half as the release 1
   confirmation, with the Opus half recorded as not run (cost) and a
   DECISIONS.md entry that amends (m). The lock is unchanged; it still names
   Opus 5.5 as a reference, so the write-up must say plainly that the second
   reference is missing.
2. **Astra now, Opus later.** Publish the Astra half as a partial
   confirmation and leave the Opus half pending until the owner funds it.
   Everything for it is prepared (above).

Either way, no new Astra runs are needed.

## Next steps

1. Get the owner's answer to the open decision.
2. Add a DECISIONS.md entry covering: the voided 2026-11-03 Opus run (cause,
   the fail-open pre-flight and missed infra guard, PR #288's fix), and the
   owner's 2026-11-10 decision on the Opus half. Follow the existing entry
   format (Decision, Evidence, Revisit triggers, What this touched).
3. Results PR (results only, its own branch off `origin/main`, owner's OK
   before opening and before merging): a confirmation write-up for
   VulcanBench Frontier v5 release 1 under `docs/results/`, with
   - a per-task table for GPT-6 Astra: attempt minutes, reward, what failed
     (from each trial's `result.json`, `verifier/` output and agent log);
   - the headline as complete passes over 21 with uncertainty (0 of 21 for
     Astra; give an exact binomial interval, for example Clopper-Pearson
     95%, not just the point estimate);
   - the Opus half's status per the owner's decision;
   - the caveats: CLI11 held only by held-out guards; pulldown-cmark
     admitted under (g') with floor 8 unmet; the prettier tamper-scan fix
     after its gate; the tree-sitter Astra gate half ran on an earlier build
     of the v5 sandbox images (`images_note` in the lock); and, if an Opus
     half is ever added, that it ran on a second Claude account at a later
     tag than the Astra half, where only `run_gate.sh` differs.
   - ferrycore: per-task numbers only, nothing from its task directory.
4. Follow-up, separate engine PR if the owner wants it:
   `scripts/computer-use/run_gate.sh` has the same fail-open usage pre-flight
   that PR #288 fixed for Frontier v5 (its "no rate-limit event, continuing"
   branch).

## If an Opus half is run later

Owner approval first, for the 21 attempts specifically. Then, from the run
worktree:

```
export CLAUDE_GATE_ENV=~/.config/vulcanbench/claude-gate-alt.env
GATE_PREFLIGHT_ONLY=1 bash scripts/frontier-v5/run_gate.sh opus
```

It must print a real usage number, `"status": "allowed"` and
`"would_start": true`; otherwise stop and tell the owner. Never read, print,
copy or log the credential file. An `ANTHROPIC_API_KEY` credential (with
`FABLE_AUTH=api`) has no usage windows and needs `GATE_USAGE_OVERRIDE=1`,
which is an owner decision. Launch with
`~/vulcanbench-confirm/bin/confirm_runs.sh opus` in the background, and about
5 minutes in open the first attempt's agent log
(`~/vulcanbench-confirm/jobs/opus-treesitter-query-anchor-matching-confirm/*/agent/claude-code.txt`)
to confirm real turns on claude-opus-5-5 with tool calls. Audit every attempt
as the Astra half was: model on every turn, real tool calls, no WebSearch,
WebFetch or MCP calls, full verifier output, tamper scan clean, and a diff
review of any solve.

## Host notes (Mac Studio)

- The shell is zsh. Run long commands in the background.
- Tests: the main checkout's venv works for running another worktree's tests:
  `PATH="$HOME/Documents/GitHub/VulcanBench/.venv/bin:$PATH" pytest -q --no-cov tests/<file>`
  (the full suite enforces 80% coverage, so `--no-cov` for single files). The
  `PATH` entry must be absolute.
- Docker is shared between sessions: every job runs under
  `~/.vulcanbench/docker-lock.sh acquire|release <name>` (the launcher does
  this).
- Harbor 0.24.0 fails a trial on any non-numeric value in `reward.json`.
- When auditing Codex rollouts, JWT-shaped (`eyJ...`) and `sk-`-shaped
  strings inside `encrypted_content` are encrypted reasoning blobs, not
  leaked tokens; resolve the JSON path before calling something a leak.
- iCloud can leave `... 2` duplicate files in checkouts; scan for them
  before a run.
