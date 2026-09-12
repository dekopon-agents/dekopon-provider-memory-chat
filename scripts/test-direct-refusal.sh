#!/usr/bin/env bash
# An empty Wasmtime linker must refuse this component: its only authority is the broker-owned
# JSONL import, and nothing outside a broker can supply it.
#
# `dekopon-run` used to make the same point three more ways (inspect, invoke, shell). It is
# retired at 0.11.1 and cannot load a 0.13.0 manifest at all, so the ground it covered now
# belongs to tests/broker.rs, which drives the real host through the testkit's FakeBroker.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/memory-chat-provider.wasm"}
[[ "$(wasmtime --version)" == 'wasmtime 48.0.2' ]]
temporary=$(mktemp -d); trap 'rm -rf "$temporary"' EXIT
if wasmtime --invoke 'describe()' "$component" >"$temporary/wasmtime.out" 2>"$temporary/wasmtime.err"; then
  echo 'error: empty Wasmtime linker unexpectedly accepted JSONL storage' >&2; exit 1
fi
grep -Fq 'dekopon:storage/jsonl@0.1.0' "$temporary/wasmtime.err"
grep -Fq 'imports instance' "$temporary/wasmtime.err"
printf 'verified direct refusal: empty Wasmtime linker cannot instantiate the component\n'
