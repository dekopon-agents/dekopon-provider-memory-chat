#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P); cd "$root"
[[ -z "${CARGO_TARGET_DIR-}" ]] || { echo 'error: CARGO_TARGET_DIR must be unset' >&2; exit 1; }
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --locked --all-targets -- -D warnings
cargo +1.98.1 test --locked --lib
cargo +1.98.1 check --locked --target wasm32-unknown-unknown
cargo +1.98.1 clippy --locked --target wasm32-unknown-unknown --lib -- -D warnings
./scripts/check-dependencies.sh
./scripts/assert-wit-sources.sh
cargo deny check licenses advisories bans sources
test -z "$(git ls-files '*.wasm')"
git diff --check
bash -n build.sh scripts/*.sh
shellcheck build.sh scripts/*.sh
python3 -m py_compile scripts/*.py
ruby -e 'require "yaml"; ARGV.each { |path| YAML.safe_load(File.read(path), aliases: true) }' .github/workflows/*.yml
actionlint -color
./scripts/validate-workflows.sh
printf 'source, native, Wasm check, dependency, license, WIT, shell, action, and no-tracked-Wasm gates passed\n'
