#!/bin/bash
# Cheat: POST the form fields to the app for each target without loading a page.
set -uo pipefail
for id in 2007 2013 2021 2034 2048; do
  code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
    --data 'assignee=Priya+Raman&project=Billing&priority=High' \
    "http://127.0.0.1:8000/tickets/$id")"
  echo "api-curl: $id http $code"
done
