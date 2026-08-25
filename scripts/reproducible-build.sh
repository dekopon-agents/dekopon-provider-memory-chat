#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
[[ -z "$(git -C "$root" status --porcelain --untracked-files=no)" ]] || { echo 'error: tracked files must be clean' >&2; exit 1; }
[[ -z "$(git -C "$root" ls-files '*.wasm')" ]]
temporary=$(mktemp -d "${TMPDIR:-/tmp}/memory-chat-repro.XXXXXX"); trap 'rm -rf "$temporary"' EXIT
mkdir "$temporary/a" "$temporary/b"
git -C "$root" archive --format=tar HEAD | tar -xf - -C "$temporary/a"
git -C "$root" archive --format=tar HEAD | tar -xf - -C "$temporary/b"
(cd "$temporary/a" && ./scripts/build-component.sh)
(cd "$temporary/b" && ./scripts/build-component.sh)
core=target/wasm32-unknown-unknown/release/dekopon_memory_chat_provider.wasm
cmp "$temporary/a/$core" "$temporary/b/$core"
cmp "$temporary/a/memory-chat-provider.wasm" "$temporary/b/memory-chat-provider.wasm"
cmp "$temporary/a/memory-chat-provider.wasm.sha256" "$temporary/b/memory-chat-provider.wasm.sha256"
printf 'two independent clean archives reproduced core and component sha256 %s\n' "$(awk '{print $1}' "$temporary/a/memory-chat-provider.wasm.sha256")"
