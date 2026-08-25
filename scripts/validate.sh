#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
"$root/scripts/validate-source.sh"
"$root/scripts/build-component.sh"
"$root/scripts/inspect-component.sh"
"$root/scripts/test-direct-refusal.sh"
DEKOPON_MEMORY_CHAT_COMPONENT="$root/memory-chat-provider.wasm" cargo +1.97.0 test --locked --test broker -- --nocapture
"$root/scripts/prepare-release-assets.sh" 0.1.0 "$root/dist"
"$root/scripts/verify-release-assets.sh" "$root/dist"
printf 'all source, component, direct-refusal, broker/storage, resource, and release-layout gates passed\n'
