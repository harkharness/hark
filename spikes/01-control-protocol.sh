#!/usr/bin/env bash
# Spike 1: discover how the Claude CLI surfaces tool-permission requests over
# the stream-json protocol, and how to answer them (allow/deny) from stdin.
#
# Usage: ./spikes/01-control-protocol.sh [extra claude flags...]
# Output: spikes/out/run.jsonl (every stdout line) + spikes/out/run.err
set -u

DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/out"
mkdir -p "$OUT"
FIFO="$OUT/stdin.fifo"
RUN="$OUT/run.jsonl"
rm -f "$FIFO" "$RUN"
mkfifo "$FIFO"

claude -p \
  --input-format stream-json \
  --output-format stream-json \
  --verbose \
  --model sonnet \
  --setting-sources "" \
  --tools "Bash" \
  "$@" \
  <"$FIFO" >"$RUN" 2>"$OUT/run.err" &
CLAUDE_PID=$!
# Kill the whole process group on exit so no `tail -f` or claude lingers.
trap 'kill "$CLAUDE_PID" 2>/dev/null; pkill -P $$ 2>/dev/null' EXIT

exec 3>"$FIFO" # hold the write end open

send() {
  printf '%s\n' "$1" >&3
  echo ">>> SENT: $1"
}

# Optional client->CLI initialize handshake (the Agent SDK does this to declare
# a canUseTool callback; without it the CLI may auto-deny instead of asking).
if [ "${INIT:-0}" = "1" ]; then
  send '{"type":"control_request","request_id":"hark-init-1","request":{"subtype":"initialize"}}'
fi

PROMPT="${PROMPT:-Run the bash command \`echo hark-spike-ok\` and tell me its output verbatim.}"
send "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"$PROMPT\"}]}}"

# Watch output; auto-answer any permission control_request with allow.
deadline=$((SECONDS + 90))
answered=0
tail -n +1 -f "$RUN" | while IFS= read -r line; do
  type=$(jq -r '.type // empty' <<<"$line" 2>/dev/null)
  case "$type" in
    control_request)
      echo "<<< CONTROL_REQUEST: $line"
      req_id=$(jq -r '.request_id // .requestId // empty' <<<"$line")
      subtype=$(jq -r '.request.subtype // empty' <<<"$line")
      if [ "$subtype" = "can_use_tool" ] && [ "$answered" -eq 0 ]; then
        answered=1
        if [ "${ACTION:-allow}" = "deny" ]; then
          send "{\"type\":\"control_response\",\"response\":{\"subtype\":\"success\",\"request_id\":\"$req_id\",\"response\":{\"behavior\":\"deny\",\"message\":\"User rejected this action from Hark.\"}}}"
        else
          send "{\"type\":\"control_response\",\"response\":{\"subtype\":\"success\",\"request_id\":\"$req_id\",\"response\":{\"behavior\":\"allow\"}}}"
        fi
      fi
      ;;
    control_response) echo "<<< CONTROL_RESPONSE: $line" ;;
    system) echo "<<< SYSTEM: $(jq -c '{subtype}' <<<"$line")" ;;
    assistant) echo "<<< ASSISTANT: $(jq -c '[.message.content[]? | {type, name}]' <<<"$line")" ;;
    result)
      echo "<<< RESULT: $(jq -c '{subtype, result, total_cost_usd, permission_denials}' <<<"$line")"
      kill "$CLAUDE_PID" 2>/dev/null
      break
      ;;
    *) echo "<<< OTHER($type)" ;;
  esac
  if [ "$SECONDS" -ge "$deadline" ]; then
    echo "TIMEOUT"
    kill "$CLAUDE_PID" 2>/dev/null
    break
  fi
done

exec 3>&-
wait "$CLAUDE_PID" 2>/dev/null
echo "--- raw stderr ---"
cat "$OUT/run.err"
