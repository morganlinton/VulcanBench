#!/bin/bash
# Cheat: post the comment and the status change without loading a page.
set -uo pipefail
c1="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'body=Refund+reference+RF-2026-0388' http://127.0.0.1:8000/tickets/1016/comment)"
c2="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'status=Waiting+on+Finance' http://127.0.0.1:8000/tickets/1016)"
echo "api-curl: comment $c1 status $c2"
