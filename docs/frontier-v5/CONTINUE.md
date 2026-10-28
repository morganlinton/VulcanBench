# Frontier v5: continuing on another host

State and setup for picking Frontier v5 up on a fresh machine (written
2026-10-21 when moving from the M2 Pro Mac to the Mac Studio). Read this,
then NIGHT-STATUS.md and GATE-LOG.md.

## Where things stand

- Composition v2 (PLAN-v2.md, frozen): 26 tasks; family F5, concentrated
  multi-bug correctness, has six slots, one per language.
- Every F5 slot got a candidate that met every F5 floor in its task image
  (2026-10-08 to 10-14).
- The admission gate's first wave (2026-10-18, GPT-6 Astra half only)
  rejected five of the six: nx-digraph, geos, ioverlay, tseslint and
  commons-lang were each solved two of two in 4 to 12 minutes of agent time.
  Only treesitter-query-anchor-matching held (Astra 0 of 3). Every solve was
  diff-reviewed as genuine. Details: GATE-LOG.md.
- The Fable 5.1 half ran on tree-sitter 2026-10-22 (Mac Studio): 1 of 3,
  then 1 of 2 in the n=5 top-up, so Fable finished 2 of 5, at the admit bar.
  Both solves diff-reviewed as genuine. **treesitter-query-anchor-matching is
  admitted as F5-c**, the first admitted task of Frontier v5 (GATE-LOG.md).
  The other five candidates are rejected on the Astra half; a Fable run on
  them only decides Routine v1 recycling (PLAN.md section 3).
- The first F2 candidate, comrak-gfm-tables-autolinks-parity (F2-rust), was
  rejected on the Astra half 2026-10-22: 2 of 2 in 11 and 8 agent minutes,
  every one of 5,410 parity cases matched, both solves genuine ports. Both
  agents compiled the shipped C reference inside the sandbox and used it as
  a differential oracle (GATE-LOG.md).
- **black-fmt-skip-region-handling is admitted as F5-python (2026-10-25)**,
  the first wave-1 task and the first to hold both references at 0 of 3:
  Astra 6 minutes per attempt, Opus 5.5 13 to 38 minutes, every guard green,
  every attempt missing the same held-out case. The Fable half was stopped at
  94% of the weekly window and the owner made Claude Opus 5.5 the wave-1
  second reference (DECISIONS.md 2026-10-25); `run_gate.sh opus` runs it and
  a usage pre-flight refuses to start above GATE_USAGE_MAX. tree-sitter's
  second prong is to be re-run under Opus 5.5 for comparability.

## Open decisions (owner)

1. ~~**Fable 5.1 access**~~ Resolved 2026-10-22: the owner's Claude
   subscription runs Fable 5.1 with a `claude setup-token` token and no usage
   credits (see "Running the gate").
2. ~~**F5 floors**~~ Decided 2026-10-26: floor 8 is amended so a candidate
   meets it by a guarded fix (route a) or by combination coverage (route b);
   PLAN-v3.md section 2, with the measurement on five tasks. Black meets both
   routes; pulldown-cmark and sqlparser-rs meet neither, so whether a
   floor-8 miss that holds the gate may be admitted is PLAN-v3 decision (g').
3. Still open from v1: (d) publishing channel, (f) gold location.
4. **F2 shape** after comrak: DESIGN.md's revisit rule says the slot should
   take a larger subsystem, and the sandbox's C toolchain let both agents
   turn the shipped reference into an oracle. Decide whether F2 ships the
   reference in a form the sandbox cannot build, or accepts measuring
   porting speed against an oracle, before sourcing the next F2 candidate.

## Setting up the host

1. Clone and check out main:
   `git clone https://github.com/morganlinton/VulcanBench.git ~/dev/VulcanBench`
2. Docker Desktop running, then build the v5 sandbox images and install
   Harbor 0.24.0: `scripts/frontier-v5/setup-host.sh` (builds the
   architecture-neutral `vulcanbench/sandbox:v5-base`, `-cfamily`, `-jvm`,
   `-rust` tags for the host CPU).
3. Tools the scripts expect: `uv` (Python 3.12 for probe_verifier.py and
   run_controls.py), `gh`, git and git-lfs. Node and Rust are only needed
   inside images. The global git config on these Macs sets
   `filter.lfs.required`, so command-line git fails outright (`git status`
   included) until `brew install git-lfs` has run (hit on the Mac Studio,
   2026-10-22).
4. Docker Desktop's VM memory must cover every task container that runs at
   once: at least 8 GiB for a compiled or JVM task at concurrency 1 (they
   declare 6 GiB), and at least 16 GiB before a Python or JavaScript task
   runs two at a time under decision (e) (tseslint declares 8 GiB per
   container). The Mac Studio came with 7.75 GiB allotted (2026-10-22); raise
   it in Docker Desktop settings before any `-n 2` run.
5. Credentials, created by the owner on the new host, never copied through
   the repo:
   - GPT-6 Astra: `codex login` (writes `~/.codex/auth.json`).
   - Fable 5.1: `claude setup-token`, saved to
     `~/.config/vulcanbench/claude-gate.env` as one line
     `CLAUDE_CODE_OAUTH_TOKEN=...` with mode 600. Paste the token at a hidden
     prompt rather than through a command line or the clipboard; a token
     copied across a terminal line wrap was cut short once (85 of 108
     characters). Or put `ANTHROPIC_API_KEY=...` there and use `FABLE_AUTH=api`.

## Running the gate

`scripts/frontier-v5/run_gate.sh <astra|fable> <task> [<task> ...]` runs a
reference on each task, three attempts, with the wave-1 identifiers,
decision (e) concurrency, the early exit at two solves, and a stop on known
infrastructure failures. Output goes to `~/vulcanbench-gate` (outside the
repo). `GATE_ATTEMPTS` (default 3) and `GATE_JOB_SUFFIX` (default empty)
exist for the n=5 top-up, which runs two more attempts into a fresh job
directory, for example:

    GATE_ATTEMPTS=2 GATE_JOB_SUFFIX=-topup scripts/frontier-v5/run_gate.sh fable treesitter-query-anchor-matching

Queued on the owner's approval: tree-sitter's second prong under Opus 5.5
(`run_gate.sh opus treesitter-query-anchor-matching`, three attempts, for
comparability with black). Wave 1 built so far (WAVE1-F5.md): black
(admitted), pulldown-cmark (Astra 0 of 3, floor 8 not met, Opus half not
run), the sqlparser-rs reserve (floor 8 not met, not gated) and
prettier-comment-attachment-stability (F5-javascript, branch
`frontier-v5-prettier`: floors 1 to 8 met by route (a); **admitted** as
F5-javascript on 2026-10-27, Astra 0 of 3 and Opus 5.5 0 of 3 (run one
attempt at a time, docs/DECISIONS.md 2026-10-27). Before it is frozen, its
tamper scan needs a fix: `\bexpect\b` flags `// @ts-expect-error` comments,
a prettier idiom (GATE-LOG); no gate reward changed). Then Java and C++. The Fable half on the five
Astra-rejected candidates (Routine v1 recycling) stays optional.

After each task: diff-review every solve against the task's base (PHASE1.md),
then record the result in GATE-LOG.md. Back up the job directories to the
private `vulcanbench-data-backup` repo the same way as the first wave.

## Evidence and history

- First-wave raw evidence (all Harbor job directories, launcher scripts and
  logs): private repo `morganlinton/vulcanbench-data-backup`,
  `archives/frontier-v5-gate-2026-10-18.tar.gz.part-*`, restore with
  `cat archives/<name>.tar.gz.part-* | tar -xzf - -C <destination>`.
- Fable half raw evidence (both tree-sitter job directories, Harbor logs and
  the run_gate.sh console logs, the whole `~/vulcanbench-gate` tree): same
  repo, `archives/frontier-v5-gate-2026-10-22-fable.tar.gz.part-*`.
- comrak Astra run: same repo,
  `archives/frontier-v5-gate-2026-10-22-astra-comrak.tar.gz.part-*`.
- Validation and control tooling: `scripts/frontier-v5/probe_verifier.py`
  (base 0, gold 1, cheating probes 0) and `scripts/frontier-v5/run_controls.py`
  (F5 single-fix and leave-one-out controls).

## Traps learned the hard way

- `harbor run`: `-k` is attempts, `-n` is concurrency. Model ids are bare
  (`claude-fable-5-1`, `gpt-6-astra`); Claude Code rejects a provider prefix.
- Never pass flag values with `--ae`: Harbor scrubs every `--ae` value from
  the files it writes.
- `logs/` is not git-ignored in this repo; keep gate and run output outside
  the repo, and stage explicit paths, never a whole tasks directory.
- Check a new reference's first attempt within minutes for real work
  (model name in the session, tool calls): infrastructure failures can look
  like an ordinary failed attempt with reward 0.
- Probes must compile (a `no_std` crate needs `extern crate std` in an exit
  probe); a probe that fails to build proves nothing.
- vitest can leave a case "pending" (no result); the typescript-eslint
  verifier re-runs only those files and never retries a failure.
- The F1 sanitizer track (OpenAPV, libmbus scaffolds) is withdrawn and stays
  untracked; the libmbus scaffold exists only on the old Mac and is not
  needed.
