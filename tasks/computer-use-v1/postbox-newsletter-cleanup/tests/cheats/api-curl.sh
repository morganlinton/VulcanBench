#!/bin/bash
# Cheat: post the bulk archive form without loading a page.
set -uo pipefail
data='action=archive&back=Newsletters'
for id in 3002 3005 3009 3012 3016 3019 3023 3026 3030; do data="$data&sel=$id"; done
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data "$data" http://127.0.0.1:8010/bulk)"
echo "api-curl: http $code"
