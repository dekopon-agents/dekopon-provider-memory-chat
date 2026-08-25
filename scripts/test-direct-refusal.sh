#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/memory-chat-provider.wasm"}
[[ "$(dekopon-run --version)" == 'dekopon-run 0.11.1' ]]
[[ "$(wasmtime --version)" == 'wasmtime 48.0.0' ]]
temporary=$(mktemp -d); trap 'rm -rf "$temporary"' EXIT
expect_refusal() {
  local name=$1; shift
  if "$@" >"$temporary/$name.out" 2>"$temporary/$name.err"; then
    echo "error: direct $name unexpectedly accepted JSONL storage" >&2; exit 1
  fi
  grep -Fq 'could not instantiate provider component' "$temporary/$name.err"
}
expect_refusal inspect dekopon-run inspect --provider "$component"
expect_refusal invoke dekopon-run invoke --provider "$component" memory.chat.recent --input '{"last":1}'
expect_refusal shell dekopon-run shell --provider "$component" 'memory recent --last 1'
if wasmtime --invoke 'describe()' "$component" >"$temporary/wasmtime.out" 2>"$temporary/wasmtime.err"; then
  echo 'error: empty Wasmtime linker unexpectedly accepted JSONL storage' >&2; exit 1
fi
grep -Fq 'dekopon:storage/jsonl@0.1.0' "$temporary/wasmtime.err"
grep -Fq 'imports instance' "$temporary/wasmtime.err"
printf 'verified direct refusal: inspect, invoke, shell, and empty Wasmtime linker\n'
