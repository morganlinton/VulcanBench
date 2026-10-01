#!/bin/zsh
# Code quality v3.18: GPT-6.1 Sol on Frontier v4 (v3.7 protocol, own population).
# Population frozen by `python -m harness.maintenance_review_v318 prepare`.
# Runs with no solver sweep active: Grok 4.7 is held until this chain prints
# V3.18 CHAIN DONE (owner decision 2026-09-29, docs/DECISIONS.md).
cd /Users/morganlinton/dev/VulcanBench
export VB_MAINT_MODULE=harness.maintenance_review_v318
export PATH="/Users/morganlinton/.local/bin:/Users/morganlinton/.nvm/versions/node/v24.16.0/bin:$PATH"
window_end() { echo "=== JUDGING WINDOW END $(date '+%F %H:%M:%S')"; }
echo "=== JUDGING WINDOW START $(date '+%F %H:%M:%S')"
for panel in muse grok; do
  .venv/bin/python -u -m harness.maintenance_review_v3_resume calibrate --panel $panel || { echo "CALIBRATE $panel STOPPED $(date '+%F %H:%M:%S')"; window_end; exit 2; }
  echo "=== CALIBRATION $panel DONE $(date '+%F %H:%M:%S')"
done
for panel in muse grok; do
  .venv/bin/python -u -m harness.maintenance_review_v3_resume run --panel $panel || { echo "RUN $panel STOPPED $(date '+%F %H:%M:%S')"; window_end; exit 3; }
  echo "=== RUN $panel DONE $(date '+%F %H:%M:%S')"
  .venv/bin/python -u -m harness.maintenance_review_v3_resume probe --panel $panel || { echo "PROBE $panel STOPPED $(date '+%F %H:%M:%S')"; window_end; exit 4; }
  echo "=== PROBE $panel DONE $(date '+%F %H:%M:%S')"
done
.venv/bin/python -u -m harness.maintenance_review_v318 summarize
window_end
echo "=== V3.18 CHAIN DONE $(date '+%F %H:%M:%S')"
