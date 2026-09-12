#!/usr/bin/env bash
# Pin caller-owned WIT to the immutable Dekopon v0.13.0 source bytes.
#
# `wit/provider.wit` is this repository's own world and has no upstream copy to compare against:
# the in-tree `examples/providers/memory-chat` it was extracted from is gone as of v0.13.0. Its
# sha256 below is the pin. The two files under `wit/deps` are verbatim upstream bytes and are
# byte-compared against the tag with `--fetch`.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
(cd "$root" && printf '%s  %s\n' \
  f48b64d59bddc9adddf5be856a3d1971e39b078e1f3df5193512d8486dc3e106 wit/provider.wit \
  eac383801715cc62f41f7267de5c191827cfd2c45c766cda5600cfef2e1c03dd wit/deps/provider.wit \
  22bfab74a10ec74685187c1ed39eb85e0e53cb387e4a33813936ebc79129dcb2 wit/deps/storage.wit | shasum -a 256 -c -)
grep -Fqx 'package dekopon:provider@0.3.0;' "$root/wit/deps/provider.wit"
grep -Fq 'include dekopon:provider/provider-cli@0.3.0;' "$root/wit/provider.wit"
if [[ ${1-} == --fetch ]]; then
  temporary=$(mktemp -d "${TMPDIR:-/tmp}/memory-chat-wit.XXXXXX")
  trap 'rm -rf "$temporary"' EXIT
  base=https://raw.githubusercontent.com/dekopon-agents/dekopon/v0.13.0
  curl --fail --silent --show-error --location "$base/crates/dekopon-provider-sdk/wit/provider.wit" -o "$temporary/provider-dep.wit"
  curl --fail --silent --show-error --location "$base/crates/dekopon-provider-storage/wit/deps/storage.wit" -o "$temporary/storage.wit"
  cmp "$root/wit/deps/provider.wit" "$temporary/provider-dep.wit"
  cmp "$root/wit/deps/storage.wit" "$temporary/storage.wit"
fi
printf 'caller-owned WIT matches immutable Dekopon v0.13.0\n'
