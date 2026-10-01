#!/bin/zsh
# Code quality v3.18 (GPT-6.1 Sol), resumed after the Grok primary stop on
# submission-054 (2026-10-01 06:01): both attempts quoted a changed token, so
# invalidate_unrecoverable_primary (v3.14 owner precedent) marked it invalid and
# the wrapper's review stage skips it; summarize publishes 054 from Muse alone.
# Appends to the chain log the Grok 4.7 hold watches for "V3.18 CHAIN DONE".
cd /Users/morganlinton/dev/VulcanBench
export VB_MAINT_MODULE=harness.maintenance_review_v318
export PATH="/Users/morganlinton/.local/bin:/Users/morganlinton/.nvm/versions/node/v24.16.0/bin:$PATH"
window_end() { echo "=== JUDGING WINDOW END $(date '+%F %H:%M:%S')"; }
echo "=== JUDGING WINDOW START $(date '+%F %H:%M:%S') (resume)"
.venv/bin/python -u -m harness.maintenance_review_v3_resume run --panel grok || { echo "RUN grok STOPPED $(date '+%F %H:%M:%S')"; window_end; exit 3; }
echo "=== RUN grok DONE $(date '+%F %H:%M:%S')"
.venv/bin/python -u -m harness.maintenance_review_v3_resume probe --panel grok || { echo "PROBE grok STOPPED $(date '+%F %H:%M:%S')"; window_end; exit 4; }
echo "=== PROBE grok DONE $(date '+%F %H:%M:%S')"
.venv/bin/python -u -m harness.maintenance_review_v318 summarize
window_end
echo "=== V3.18 CHAIN DONE $(date '+%F %H:%M:%S')"
