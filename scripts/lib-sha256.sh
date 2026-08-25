#!/usr/bin/env bash
# shellcheck shell=bash

sha256_file() {
  local file=${1:?sha256_file requires a path}
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  else
    shasum -a 256 "$file" | awk '{print $1}'
  fi
}

write_sha256() {
  local file=${1:?write_sha256 requires a path}
  local output=${2:?write_sha256 requires an output path}
  printf '%s  %s\n' "$(sha256_file "$file")" "$(basename "$file")" >"$output"
}

check_sha256() {
  local checksum=${1:?check_sha256 requires a checksum path}
  local directory
  directory=$(cd "$(dirname "$checksum")" && pwd -P)
  local expected name actual
  read -r expected name <"$checksum"
  [[ "$expected" =~ ^[0-9a-f]{64}$ && "$name" == memory-chat-provider.wasm ]] || {
    echo "error: malformed memory-chat provider checksum" >&2
    return 1
  }
  actual=$(sha256_file "$directory/$name")
  [[ "$actual" == "$expected" ]] || {
    echo "error: checksum mismatch for $directory/$name" >&2
    return 1
  }
}
