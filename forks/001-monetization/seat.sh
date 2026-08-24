#!/usr/bin/env zsh
# seat.sh <gpt|gemini> <prompt-file> <out-prefix> [--search]
# Reads keys from ~/.zshrc (unexported there). Never prints keys.
set -u
source ~/.zshrc >/dev/null 2>&1
seat=$1; promptfile=$2; out=$3; search=${4:-}
sys="You are one seat in a multi-model design boardroom for the Zurfur project. Argue independently and bluntly; no cheerleading; do not converge for politeness. Keep your visible answer under 1500 words."

case $seat in
gpt)
  tools='[]'
  if [[ $search == --search ]]; then tools='[{"type":"web_search"}]'; fi
  jq -n --arg sys "$sys" --rawfile p "$promptfile" --argjson tools "$tools" \
    '{model:"gpt-5.6-sol", max_completion_tokens:8000,
      messages:[{role:"system",content:$sys},{role:"user",content:$p}]}
      + (if ($tools|length)>0 then {tools:$tools} else {} end)' > "$out.req.json"
  curl -sS -m 600 https://api.openai.com/v1/chat/completions \
    -H "Authorization: Bearer $OPENAI_API_KEY" -H 'Content-Type: application/json' \
    -d @"$out.req.json" > "$out.raw.json"
  jq -r '.choices[0].message.content // ("API-ERROR: " + (.error.message // "unknown"))' "$out.raw.json" > "$out.md"
  jq -c '{usage: .usage | {prompt_tokens, completion_tokens}}' "$out.raw.json" 2>/dev/null || true
  ;;
gemini)
  tools='[]'
  if [[ $search == --search ]]; then tools='[{"google_search":{}}]'; fi
  jq -n --arg sys "$sys" --rawfile p "$promptfile" --argjson tools "$tools" \
    '{system_instruction:{parts:[{text:$sys}]},
      contents:[{parts:[{text:$p}]}],
      generationConfig:{maxOutputTokens:8000}}
      + (if ($tools|length)>0 then {tools:$tools} else {} end)' > "$out.req.json"
  curl -sS -m 600 "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-pro-preview:generateContent" \
    -H "x-goog-api-key: $GEMINI_API_KEY" -H 'Content-Type: application/json' \
    -d @"$out.req.json" > "$out.raw.json"
  jq -r '[.candidates[0].content.parts[]? | select(.text) | .text] | join("\n") | if . == "" then "API-ERROR: empty" else . end' "$out.raw.json" > "$out.md" 2>/dev/null \
    || jq -r '"API-ERROR: " + (.error.message // "unknown")' "$out.raw.json" > "$out.md"
  jq -c '{usage: .usageMetadata | {promptTokenCount, candidatesTokenCount, thoughtsTokenCount}}' "$out.raw.json" 2>/dev/null || true
  ;;
esac
