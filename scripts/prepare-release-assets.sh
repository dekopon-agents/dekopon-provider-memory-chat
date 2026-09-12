#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
manifest_version=$(awk -F'"' '/^version = /{print $2; exit}' "$root/Cargo.toml")
version=${1:-$manifest_version}; destination=${2:-"$root/dist"}
[[ "$version" == "$manifest_version" ]] || { echo "error: $version is not the manifest version $manifest_version" >&2; exit 1; }
case "$destination" in "$root/dist"|"$root"/dist/*) ;; *) echo 'error: destination must be under dist/' >&2; exit 1;; esac
[[ -f "$root/memory-chat-provider.wasm" && -f "$root/memory-chat-provider.wasm.sha256" ]]
(cd "$root" && shasum -a 256 -c memory-chat-provider.wasm.sha256)
rm -rf "$destination"; mkdir -p "$destination"
cp "$root/memory-chat-provider.wasm" "$root/memory-chat-provider.wasm.sha256" "$destination/"
"$root/scripts/verify-release-assets.sh" "$destination"
