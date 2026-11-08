#!/bin/bash
# Verifier entry point. Writes the reward map harbor reads; always exits 0 so a
# checker crash is recorded as reward 0 rather than a verifier error.
mkdir -p /logs/verifier
if ! python3 /check.py /var/lib/postbox /seed-postbox.json /arrivals-postbox.json > /logs/verifier/reward.json 2> /logs/verifier/check.log; then
  echo '{"reward": 0, "verifier_crash": 1}' > /logs/verifier/reward.json
fi
cat /logs/verifier/reward.json
exit 0
