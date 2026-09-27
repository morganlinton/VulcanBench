#!/usr/bin/env bash
# Verify the JavaScript, Rust, C++ and C calibration controls (MULTILANG.md):
# every control reproduces the shared ledger behaviour, and the controls that
# are defined as copies of another (2 of 1, 6 of 2, 5, 7 and 8 of 0) are.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
status=0
for language in javascript rust cpp c; do
  if (cd "$here/$language" && bash verify.sh >/dev/null 2>&1); then
    echo "ok    $language"
  else
    echo "FAIL  $language (run $language/verify.sh for detail)"
    status=1
  fi
done
exit "$status"
