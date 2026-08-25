#!/usr/bin/env bash
# Pin caller-owned WIT to the immutable Dekopon v0.11.1 source bytes.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
(cd "$root" && printf '%s  %s\n' \
  cc50361e91cb27fa466b49f0fddf3fe5084cf84a58e40f167678cd828a243f17 wit/provider.wit \
  02ba5a92067f53bc8f48e10bf221229c5b7f33f791a031741da5011c32ab37c9 wit/deps/provider.wit \
  22bfab74a10ec74685187c1ed39eb85e0e53cb387e4a33813936ebc79129dcb2 wit/deps/storage.wit | shasum -a 256 -c -)
if [[ ${1-} == --fetch ]]; then
  temporary=$(mktemp -d "${TMPDIR:-/tmp}/memory-chat-wit.XXXXXX")
  trap 'rm -rf "$temporary"' EXIT
  base=https://raw.githubusercontent.com/dekopon-agents/dekopon/v0.11.1
  curl --fail --silent --show-error --location "$base/examples/providers/memory-chat/wit/provider.wit" -o "$temporary/provider.wit"
  curl --fail --silent --show-error --location "$base/crates/dekopon-provider-sdk/wit/provider.wit" -o "$temporary/provider-dep.wit"
  curl --fail --silent --show-error --location "$base/crates/dekopon-provider-storage/wit/deps/storage.wit" -o "$temporary/storage.wit"
  cmp "$root/wit/provider.wit" "$temporary/provider.wit"
  cmp "$root/wit/deps/provider.wit" "$temporary/provider-dep.wit"
  cmp "$root/wit/deps/storage.wit" "$temporary/storage.wit"
fi
printf 'caller-owned WIT matches immutable Dekopon v0.11.1\n'
