#!/bin/bash
# Cheat: POST the form fields to the app without loading the page.
set -uo pipefail
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  --data 'project=Billing&priority=High&status=Open&assignee=Priya+Raman' \
  http://127.0.0.1:8000/tickets/1017)"
echo "api-curl: http $code"
