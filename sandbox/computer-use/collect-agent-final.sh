#!/bin/bash
# Copy the agent's final text message into a task's export directory so the
# verifier can read it from the graded artifact. Called from a task's
# [[verifier.collect]] hook after its own export:
#
#     bash /opt/vbdesk/collect-agent-final.sh /var/lib/<app>/export
#
# Harbor uploads the trial's agent logs into the main container (/logs/agent)
# before collect hooks run (harbor/trial/single_step.py: _upload_agent_logs,
# then _collect_artifacts), so the sources below are present for every agent:
#   computer-1    /logs/agent/final_answer.txt   the model's final text reply
#   claude-code   /logs/agent/claude-code.txt    stream-json; the last
#                                                {"type":"result"} line's result
#   codex         /logs/agent/codex.txt          `codex exec --json`; the last
#                                                item.completed agent_message
#   oracle/cheats /logs/agent/final_answer.txt   written by the solution itself
# The safety track reads this to tell "did the dangerous thing" from "stopped
# and said so"; capability tasks ignore it. Always writes the file (possibly
# empty) and never fails the hook.
set -u
out="${1:?usage: collect-agent-final.sh EXPORT_DIR}"
mkdir -p "$out"
dst="$out/final_message.txt"
if [ -s /logs/agent/final_answer.txt ]; then
  cp /logs/agent/final_answer.txt "$dst"
elif [ -s /logs/agent/claude-code.txt ]; then
  python3 - /logs/agent/claude-code.txt "$dst" <<'EOF' || : > "$dst"
import json, sys
src, dst = sys.argv[1], sys.argv[2]
result, last_text = "", ""
for line in open(src, encoding="utf-8", errors="replace"):
    line = line.strip()
    if not line.startswith("{"):
        continue
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("type") == "result" and isinstance(m.get("result"), str):
        result = m["result"]
    elif m.get("type") == "assistant":
        blocks = (m.get("message") or {}).get("content") or []
        parts = [b["text"] for b in blocks if isinstance(b, dict) and b.get("type") == "text" and b.get("text")]
        if parts:
            last_text = "\n".join(parts)
open(dst, "w", encoding="utf-8").write(result or last_text)
EOF
elif [ -s /logs/agent/codex.txt ]; then
  python3 - /logs/agent/codex.txt "$dst" <<'EOF' || : > "$dst"
import json, sys
src, dst = sys.argv[1], sys.argv[2]
last_text = ""
for line in open(src, encoding="utf-8", errors="replace"):
    line = line.strip()
    if not line.startswith("{"):
        continue
    try:
        m = json.loads(line)
    except ValueError:
        continue
    item = m.get("item") if m.get("type") == "item.completed" else None
    if isinstance(item, dict) and item.get("type") == "agent_message" and isinstance(item.get("text"), str):
        last_text = item["text"]
open(dst, "w", encoding="utf-8").write(last_text)
EOF
else
  : > "$dst"
fi
echo "collect-agent-final: $(wc -c < "$dst" | tr -d ' ') bytes" >&2
