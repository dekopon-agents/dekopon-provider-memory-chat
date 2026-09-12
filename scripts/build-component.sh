#!/usr/bin/env bash
# Build the deterministic JSONL-importing release component in this checkout's ordinary target/.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/memory-chat-provider.wasm"}
core="$root/target/wasm32-unknown-unknown/release/dekopon_memory_chat_provider.wasm"
toolchain=1.98.1
required_rustc='rustc 1.98.1 (48a229cea 2026-09-01)'
required_wasm_tools='wasm-tools 1.259.0'
version=$(awk -F'"' '/^version = /{print $2; exit}' "$root/Cargo.toml")
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'error: no package version in Cargo.toml' >&2; exit 1; }
[[ -z "${CARGO_TARGET_DIR-}" ]] || { echo 'error: CARGO_TARGET_DIR must be unset' >&2; exit 1; }
[[ "$(rustup run "$toolchain" rustc --version)" == "$required_rustc" ]] || { echo "error: expected $required_rustc" >&2; exit 1; }
[[ "$(wasm-tools --version)" == "$required_wasm_tools" ]] || { echo "error: expected $required_wasm_tools" >&2; exit 1; }
[[ -z "$(git -C "$root" ls-files '*.wasm' 2>/dev/null || true)" ]] || { echo 'error: generated Wasm must not be tracked' >&2; exit 1; }
"$root/scripts/assert-wit-sources.sh"
cargo_home=${CARGO_HOME:-"$HOME/.cargo"}; cargo_home=$(cd "$cargo_home" && pwd -P)
sysroot=$(rustup run "$toolchain" rustc --print sysroot); sysroot=$(cd "$sysroot" && pwd -P)
rustflags=(
  "--remap-path-prefix=$root=/dekopon/source"
  "--remap-path-prefix=$cargo_home=/dekopon/cargo"
  "--remap-path-prefix=$sysroot=/dekopon/rust/$toolchain"
  '--cfg=dekopon_provider_repro_v1'
  '--check-cfg=cfg(dekopon_provider_repro_v1)'
)
encoded=$(printf '%s\x1f' "${rustflags[@]}"); encoded=${encoded%$'\x1f'}
mkdir -p "$(dirname "$component")"
SOURCE_DATE_EPOCH=0 LANG=C.UTF-8 LC_ALL=C CARGO_TERM_COLOR=never CARGO_ENCODED_RUSTFLAGS="$encoded" \
  cargo +"$toolchain" rustc --locked --package dekopon-memory-chat-provider \
    --target wasm32-unknown-unknown --release -- \
    -C metadata="dekopon-memory-chat-provider-$version-repro-v1" -C extra-filename=
test -s "$core"
wasm-tools validate "$core"
wasm-tools component new "$core" -o "$component"
wasm-tools validate "$component"
for local_path in "$root" "$cargo_home" "$sysroot"; do
  ! LC_ALL=C grep -aF -- "$local_path" "$component" >/dev/null || { echo "error: local path embedded: $local_path" >&2; exit 1; }
done
size=$(wc -c <"$component" | tr -d ' ')
(( size <= 1000000 )) || { echo "error: component exceeds 1,000,000 bytes: $size" >&2; exit 1; }
(cd "$(dirname "$component")" && shasum -a 256 "$(basename "$component")" >"$(basename "$component").sha256")
printf 'generated %s (%s bytes)\n' "$component" "$size"
