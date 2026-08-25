#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/memory-chat-provider.wasm"}
core="$root/target/wasm32-unknown-unknown/release/dekopon_memory_chat_provider.wasm"
test -f "$component"
wasm-tools validate "$component"
json=$(mktemp); text=$(mktemp); core_text=$(mktemp); trap 'rm -f "$json" "$text" "$core_text"' EXIT
wasm-tools component wit -j "$component" >"$json"
wasm-tools component wit "$component" >"$text"
if [[ -f "$core" ]]; then wasm-tools print "$core" >"$core_text"; else : >"$core_text"; fi
jq -e '
  (.worlds | length) == 1 and (.interfaces | length) == 1 and
  ((.worlds[0].exports | keys | sort) == ["describe","invoke","resolve-command"]) and
  ((.worlds[0].imports | length) == 1) and
  (.interfaces[0].name == "jsonl") and
  ((.interfaces[0].functions | keys | sort) == ["append","read-chunk","replace","size"]) and
  (.packages[.interfaces[0].package].name == "dekopon:storage@0.1.0") and
  (([.packages[].name] | sort) == ["dekopon:storage@0.1.0","root:component"])
' "$json" >/dev/null
if [[ -s "$core_text" ]]; then
  imports=$(grep -E '^  \(import ' "$core_text" || true)
  [[ "$(grep -c . <<<"$imports")" == 4 ]]
  for function in append read-chunk replace size; do
    grep -Fq "import \"dekopon:storage/jsonl@0.1.0\" \"$function\"" <<<"$imports"
  done
fi
if grep -Eqi 'wasi:|wasix|wasi_snapshot|dekopon:http|durable-files|filesystem|environment|random|clocks' "$text"; then
  echo 'error: forbidden ambient or non-JSONL import found' >&2; exit 1
fi
size=$(wc -c <"$component" | tr -d ' '); ((size <= 1000000))
printf 'verified %s bytes: only JSONL@0.1.0; three provider exports; no WASI/HTTP/ambient imports\n' "$size"
