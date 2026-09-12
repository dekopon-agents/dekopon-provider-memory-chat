#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P); cd "$root"
for pair in \
  'dekopon-provider-sdk 0.13.0' \
  'dekopon-provider-storage 0.13.0' \
  'serde 1.0.229' \
  'serde_json 1.0.151' \
  'wit-bindgen 0.62.0'; do
  read -r name version <<<"$pair"
  awk -v n="$name" -v v="$version" '
    $0 == "name = \"" n "\"" { seen=1; next }
    seen && $0 == "version = \"" v "\"" { found=1 }
    seen && /^$/ { seen=0 }
    END { exit !found }
  ' Cargo.lock || { echo "error: missing exact lock entry $pair" >&2; exit 1; }
done
if grep -Eq '^source = "(git\+|path\+)' Cargo.lock; then echo 'error: lock contains non-registry source' >&2; exit 1; fi
if grep -Eq '(path|git)\s*=' Cargo.toml; then echo 'error: manifest contains path/git dependency' >&2; exit 1; fi
graph=$(mktemp); trap 'rm -f "$graph"' EXIT
cargo +1.98.1 tree --locked --target wasm32-unknown-unknown --edges normal,build >"$graph"
grep -Fq 'dekopon-provider-storage v0.13.0' "$graph"
for forbidden in 'dekopon-provider-http' 'dekopon-http-host' 'wasi ' 'wasix' 'reqwest ' 'tokio '; do
  ! grep -Fiq "$forbidden" "$graph" || { echo "error: shipped graph contains $forbidden" >&2; exit 1; }
done
printf 'exact crates.io pins, JSONL-only feature graph, and lock source policy passed\n'
