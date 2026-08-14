#!/usr/bin/env bash
# Spike: validate (a) multi-turn over one spawned claude process and
# (b) image content blocks over stream-json input.
set -u

DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/out"
mkdir -p "$OUT"
FIFO="$OUT/mt.fifo"
RUN="$OUT/mt.jsonl"
rm -f "$FIFO" "$RUN"
mkfifo "$FIFO"

# Hand-craft a 1x1 red BMP (58 bytes), convert to PNG with sips (stock macOS).
BMP="$OUT/red.bmp"
PNG="$OUT/red.png"
printf '424d3a000000000000003600000028000000010000000100000001001800000000000400000000000000000000000000000000000000' | xxd -r -p >"$BMP"
printf '0000ff00' | xxd -r -p >>"$BMP"
sips -s format png "$BMP" --out "$PNG" >/dev/null
B64=$(base64 -i "$PNG" | tr -d '\n')

claude -p \
  --input-format stream-json \
  --output-format stream-json \
  --verbose \
  --model sonnet \
  --setting-sources "" \
  --tools "" \
  <"$FIFO" >"$RUN" 2>"$OUT/mt.err" &
CLAUDE_PID=$!
# Kill the whole process group on exit so no `tail -f` or claude lingers.
trap 'kill "$CLAUDE_PID" 2>/dev/null; pkill -P $$ 2>/dev/null' EXIT
exec 3>"$FIFO"

send() { printf '%s\n' "$1" >&3; }

send '{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Reply with exactly: ready"}]}}'

results=0
tail -n +1 -f "$RUN" | while IFS= read -r line; do
  type=$(jq -r '.type // empty' <<<"$line" 2>/dev/null)
  if [ "$type" = "result" ]; then
    results=$((results + 1))
    echo "RESULT#$results: $(jq -c '{subtype, result: (.result|tostring|.[0:120]), total_cost_usd}' <<<"$line")"
    if [ "$results" -eq 1 ]; then
      send "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":\"image/png\",\"data\":\"$B64\"}},{\"type\":\"text\",\"text\":\"Qual a cor predominante desta imagem? Responda apenas a cor, em portugues.\"}]}}"
    else
      kill "$CLAUDE_PID" 2>/dev/null
      break
    fi
  fi
done

exec 3>&-
wait "$CLAUDE_PID" 2>/dev/null
echo "--- stderr ---"
head -3 "$OUT/mt.err"
