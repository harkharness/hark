#!/bin/bash
# Can a turn be interrupted without killing the session?
#
# The permission channel proves client -> CLI control_request works
# (spikes/FINDINGS.md). This asks the one thing that was never tested:
# does the CLI implement subtype "interrupt", and does the process stay
# alive and answer a NEXT message afterwards?
set -uo pipefail
CLAUDE="${CLAUDE_BIN:-claude}"
OUT="$(dirname "$0")/interrupt.jsonl"
: > "$OUT"

long='{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Conte de 1 a 600, um numero por linha, sem usar ferramentas e sem parar."}]}}'
after='{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Responda apenas: VIVO"}]}}'
irq='{"type":"control_request","request_id":"irq-1","request":{"subtype":"interrupt"}}'

{
  echo "$long"
  sleep 4
  echo "$irq"
  sleep 4
  echo "$after"
  sleep 12
} | "$CLAUDE" -p \
      --model haiku \
      --input-format stream-json \
      --output-format stream-json \
      --verbose \
      --tools "" \
      --setting-sources "" \
      --system-prompt "Responda em portugues, sem preambulo." \
  > "$OUT" 2>"$OUT.err"

echo "--- exit: $? ---"
echo "--- tipos de evento ---"
python3 - "$OUT" <<'PY'
import json,sys
seen=[]
for line in open(sys.argv[1]):
    line=line.strip()
    if not line: continue
    try: e=json.loads(line)
    except Exception: continue
    t=e.get("type")
    sub=e.get("subtype") or (e.get("request") or {}).get("subtype") or (e.get("response") or {}).get("subtype")
    seen.append(f"{t}/{sub}" if sub else t)
from collections import Counter
for k,v in Counter(seen).items(): print(f"{v:3}  {k}")
print("--- control_response bruto ---")
for line in open(sys.argv[1]):
    if '"control_response"' in line: print(line.strip()[:300])
print("--- results ---")
for line in open(sys.argv[1]):
    if '"type":"result"' in line:
        e=json.loads(line)
        print("subtype:",e.get("subtype"),"| erro:",e.get("is_error"),"| turnos:",e.get("num_turns"),"| custo:",e.get("total_cost_usd"))
PY
echo "--- stderr ---"; tail -5 "$OUT.err"
