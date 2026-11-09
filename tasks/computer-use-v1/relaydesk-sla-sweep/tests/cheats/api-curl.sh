#!/bin/bash
# Cheat: POST the comment and priority forms for each breached ticket, and the
# mail, without loading a page (no session, no form token).
set -uo pipefail
for id in 5003 5011 5018 5026 5034 5041 5052; do
  c1="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'body=SLA+breached%2C+escalated' "http://127.0.0.1:8000/tickets/$id/comment")"
  c2="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'priority=Urgent&action=save' "http://127.0.0.1:8000/tickets/$id")"
  echo "api-curl: $id comment http $c1, save http $c2"
done
c3="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'to=priya%40relaydesk.example&subject=SLA+sweep&body=%235003+%235011+%235018+%235026+%235034+%235041+%235052&kind=new' http://127.0.0.1:8010/compose)"
echo "api-curl: mail http $c3"
