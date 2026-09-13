#!/usr/bin/env bash
# shellcheck disable=SC2016 # Workflow snippets below are intentionally literal.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
attestation_verifier="$root/scripts/verify-attestation-anonymously.sh"
[[ -f "$ci" && -f "$release" && -f "$attestation_verifier" ]] || {
  echo 'error: CI/release workflows and verification helpers are required' >&2
  exit 1
}

python3 - "$ci" "$release" <<'PY'
import pathlib
import re
import sys
for name in sys.argv[1:]:
    for number, line in enumerate(pathlib.Path(name).read_text().splitlines(), 1):
        match = re.match(r"\s*uses:\s*([^\s#]+)", line)
        if match and not re.fullmatch(r"[^@]+@[0-9a-f]{40}", match.group(1)):
            raise SystemExit(f"{name}:{number}: Action is not full-SHA pinned")
PY

for required in \
  'persist-credentials: false' \
  'Reject tracked Wasm' \
  './scripts/validate-source.sh' \
  './scripts/reproducible-build.sh'; do
  grep -Fq -- "$required" "$ci"
done
for required in \
  '      - "v*"' \
  'test "$GITHUB_REF_NAME" = "v$version"' \
  'git cat-file -t' \
  'subject-path: dist/memory-chat-provider.wasm' \
  'predicate-type: https://cyclonedx.org/bom' \
  'memory-chat-provider.wasm:application/wasm' \
  'application/vnd.dekopon.provider.v1+wasm' \
  'provider-memory-chat/versions' \
  'draft: false' \
  'make_latest: "false"' \
  'cleanup_failed_release:' \
  'cleanup-release-id' \
  'cleanup-release-tag' \
  'cleanup-package' \
  'failed cleanup did not restore original release/package absence' \
  'repo tags' \
  'LC_ALL=C sort' \
  'final-versions.jsons' \
  'final-owned-draft.json' \
  '/orgs/dekopon-agents/packages/container/provider-memory-chat' \
  'jq -sr --arg marker "$marker"'; do
  grep -Fq -- "$required" "$release"
done
if grep -Fq 'v0.1.0' "$release"; then
  echo 'error: release workflow is pinned to a single version' >&2
  exit 1
fi
if grep -Eq 'test "\$(version|VERSION)" = [0-9]' "$release"; then
  echo 'error: release workflow asserts a literal crate version' >&2
  exit 1
fi
if grep -Fq 'dekopon-run' "$release" "$ci"; then
  echo 'error: dekopon-run is retired and has no 0.13.0' >&2
  exit 1
fi
if grep -Fq 'jq -ser --arg marker "$marker"' "$release"; then
  echo 'error: draft visibility polling still fails on an intentionally empty result' >&2
  exit 1
fi
if grep -Fq 'repo tags "$OCI_REPOSITORY" --output json' "$release"; then
  echo 'error: ORAS 1.2.3 does not support repo tags --output' >&2
  exit 1
fi
for required in \
  'RUN_ID RUN_ATTEMPT' \
  'runInvocationURI' \
  'expected one attestation for current invocation'; do
  grep -Fq "$required" "$attestation_verifier"
done

build_job=$(sed -n '/^  build:/,/^  attest:/p' "$release")
if ! grep -Eq 'test .*git rev-parse refs/remotes/origin/main.* = .*GITHUB_SHA' \
  <<<"$build_job"; then
  echo 'error: release preflight does not require current main identity' >&2
  exit 1
fi
if grep -Fq 'git merge-base --is-ancestor' "$release"; then
  echo 'error: release tag need not equal current main' >&2
  exit 1
fi
draft_job=$(sed -n '/^  draft:/,/^  ghcr:/p' "$release")
if grep -Fq 'id-token: write' <<<"$draft_job"; then
  echo 'error: draft job has unnecessary OIDC authority' >&2
  exit 1
fi
if grep -Eq 'provider-memory-chat:(latest|staging|tmp|temp)' "$release"; then
  echo 'error: mutable OCI tag' >&2
  exit 1
fi
if grep -Eq 'cargo clean|CARGO_TARGET_DIR' "$ci" "$release"; then
  echo 'error: target policy violation' >&2
  exit 1
fi

printf 'workflow full-SHA pins, release transaction, provenance, and cleanup gates passed\n'
