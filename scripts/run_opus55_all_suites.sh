#!/usr/bin/env bash
# Claude Opus 5.5 across all three suites, one after another.
#
# Sweeps stay serial (docs/DECISIONS.md 2026-09-13: speed cards rank by wall
# clock), and each launcher refuses to start while another vulcanbench run is
# active, so the suites are chained here rather than run side by side.
#
# Order reset 2026-09-22 on the owner's call: Frontier v4 is the public
# headline board and was being blocked ~2 days by the private Safety suite,
# so it now runs first. Routine v1 already finished (60/60) and is gone from
# this script. Safety v1 resumes where it stopped: its low level is complete
# (10/10) and --only-missing skips it.
#
# Every leg runs --billing subscription: the Max plan, never metered API.
#
#   nohup bash scripts/run_opus55_all_suites.sh > logs/opus55-all-suites.log 2>&1 &
set -u

MODEL_BARE=claude-opus-5-5
MODEL_SPEC=claude-code:claude-opus-5-5
LEVELS="low medium high extra-high max"
ROUTINE=/Users/morganlinton/dev/VulcanRoutine
CONDUCT=/Users/morganlinton/dev/VulcanConduct
VB=/Users/morganlinton/dev/VulcanBench

# Refusal fallback ON (Claude Code's default), matching Artificial Analysis,
# which publishes Opus 5.5 as "(<effort>, Default Fallback)" and counts every
# run. Owner decision 2026-09-24, docs/DECISIONS.md. The fallback-off attempts
# are kept in runs-effort-opus55-nofallback-archive/. Make sure the variable
# is not inherited from the launching shell:
unset CLAUDE_CODE_DISABLE_REFUSAL_FALLBACK

banner() { echo; echo "##### $1 $(date '+%F %H:%M:%S')"; echo; }

banner "LEG 1/2 Frontier v4 (23 tasks x 5 levels)"
MODEL="$MODEL_SPEC" OUTROOT=runs-effort-opus55 PROBE="$MODEL_BARE" \
  LEVELS="$LEVELS" bash "$VB/scripts/cii-v4-board/run_effort_sweep.sh"

banner "LEG 2/2 Safety v1 / conduct-v1 (resuming; low 10/10)"
HARNESS=claude-code MODEL="$MODEL_BARE" OUTROOT=runs-conduct-v1-opus55 \
  LEVELS="$LEVELS" bash "$CONDUCT/scripts/run_conduct_sweep.sh"

banner "BOTH REMAINING SUITES FINISHED"
