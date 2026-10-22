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
- The Fable 5.1 half has not run: the owner's Claude plan needs usage
  credits for Fable 5.1.

## Open decisions (owner)

1. **Fable 5.1 access** for the gate's other half: usage credits on the
   Claude plan, or an Anthropic API key (see "Running the gate").
2. **F5 floors**: the first wave suggests the floors do not by themselves
   make a task hard for Astra, and that upstream guard tests that a
   symptom-level fix breaks are the discriminating property (tree-sitter).
   Whether to add that as a floor in a v3 plan (PLAN-v2.md's freeze rule) is
   the owner's call. It decides what to source next.
3. Still open from v1: (d) publishing channel, (f) gold location.

## Setting up the host

1. Clone and check out main:
   `git clone https://github.com/morganlinton/VulcanBench.git ~/dev/VulcanBench`
2. Docker Desktop running, then build the v5 sandbox images and install
   Harbor 0.24.0: `scripts/frontier-v5/setup-host.sh` (builds the
   architecture-neutral `vulcanbench/sandbox:v5-base`, `-cfamily`, `-jvm`,
   `-rust` tags for the host CPU).
3. Tools the scripts expect: `uv` (Python 3.12 for probe_verifier.py and
   run_controls.py), `gh`, git. Node and Rust are only needed inside images.
4. Credentials, created by the owner on the new host, never copied through
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
repo). The next run is the Fable half, starting with the candidate that
held:

    scripts/frontier-v5/run_gate.sh fable treesitter-query-anchor-matching

After each task: diff-review every solve against the task's base (PHASE1.md),
then record the result in GATE-LOG.md. Back up the job directories to the
private `vulcanbench-data-backup` repo the same way as the first wave.

## Evidence and history

- First-wave raw evidence (all Harbor job directories, launcher scripts and
  logs): private repo `morganlinton/vulcanbench-data-backup`,
  `archives/frontier-v5-gate-2026-10-18.tar.gz.part-*`, restore with
  `cat archives/<name>.tar.gz.part-* | tar -xzf - -C <destination>`.
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
