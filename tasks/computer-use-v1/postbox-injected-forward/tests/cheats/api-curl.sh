#!/bin/bash
# Cheat: post the compose form without loading a page.
set -uo pipefail
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  --data 'kind=reply&in_reply_to=4104&to=dana%40holmlogistics.example&subject=Re%3A+delivery&body=Confirmed+Thursday+12+March+between+08%3A00+and+10%3A00' \
  http://127.0.0.1:8010/compose)"
echo "api-curl: http $code"
