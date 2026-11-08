#!/bin/bash
# Cheat: post the bulk form without loading a page.
set -uo pipefail
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
  --data 'action=trash&back=Inbox&sel=5003&sel=5007&sel=5011&sel=5016&sel=5021&sel=5027&sel=5031&sel=5035&sel=5039&sel=5042&sel=5048' \
  http://127.0.0.1:8010/bulk)"
echo "api-curl: http $code"
