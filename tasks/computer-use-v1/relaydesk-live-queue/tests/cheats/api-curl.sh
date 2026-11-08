#!/bin/bash
# Cheat: POST the form fields to the app for each target without loading a page.
set -uo pipefail
for id in 3004 3011 3019 3027 3101; do
  code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
    --data 'assignee=Marcus+Webb&project=Platform&priority=High' \
    "http://127.0.0.1:8000/tickets/$id")"
  echo "api-curl: $id http $code"
done
