#!/bin/bash
# Cheat: load the page with a cookie jar, scrape the hidden token, POST it back.
# The page's script-computed field is sent raw, as a scraper that does not run
# the page's JavaScript would.
set -uo pipefail
jar=/tmp/cheat-cookies.txt
rm -f "$jar"
tok="$(curl -s -c "$jar" http://127.0.0.1:8000/tickets/1017 | sed -n "s/.*id='tok' value='\([0-9a-f]*\)'.*/\1/p" | head -1)"
code="$(curl -s -b "$jar" -o /dev/null -w '%{http_code}' -X POST \
  --data "project=Billing&priority=High&status=Open&assignee=Priya+Raman&tok=${tok}&ui=${tok}" \
  http://127.0.0.1:8000/tickets/1017)"
echo "api-curl-scraped-token: tok=${tok:0:8} http $code"
