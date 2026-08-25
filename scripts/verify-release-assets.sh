#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
directory=${1:-"$root/dist"}
[[ -d "$directory" ]]
names=()
while IFS= read -r name; do names+=("$name"); done < <(find "$directory" -mindepth 1 -maxdepth 1 -type f -exec basename {} \; | LC_ALL=C sort)
[[ ${#names[@]} -eq 2 && ${names[0]} == memory-chat-provider.wasm && ${names[1]} == memory-chat-provider.wasm.sha256 ]] || { printf 'error: unexpected assets: %s\n' "${names[*]}" >&2; exit 1; }
[[ -z "$(find "$directory" -mindepth 1 -maxdepth 1 ! -type f -print -quit)" ]]
(cd "$directory" && shasum -a 256 -c memory-chat-provider.wasm.sha256)
wasm-tools validate "$directory/memory-chat-provider.wasm"
"$root/scripts/inspect-component.sh" "$directory/memory-chat-provider.wasm"
size=$(wc -c <"$directory/memory-chat-provider.wasm" | tr -d ' '); ((size <= 1000000))
printf 'verified exact two-file release set (%s bytes)\n' "$size"
