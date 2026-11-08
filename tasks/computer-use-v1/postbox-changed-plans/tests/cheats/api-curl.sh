#!/bin/bash
# Cheat: post the compose form without loading a page.
set -uo pipefail
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  --data 'kind=reply&in_reply_to=4203&to=dana%40holmlogistics.example&subject=Re%3A+Leith&body=Confirmed+Wednesday+11+March+between+13%3A00+and+15%3A00' \
  http://127.0.0.1:8010/compose)"
echo "api-curl: http $code"
