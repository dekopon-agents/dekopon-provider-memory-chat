#!/usr/bin/env bash
# shellcheck disable=SC2016 # Workflow snippets below are intentionally literal.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
recovery="$root/.github/workflows/recover-v0.1.0.yml"
recovery_helper="$root/scripts/recover-v0.1.0-artifacts.sh"
attestation_verifier="$root/scripts/verify-attestation-anonymously.sh"
[[ -f "$ci" && -f "$release" && -f "$recovery" && -f "$recovery_helper" && \
   -f "$attestation_verifier" ]] || {
  echo 'error: CI/release/recovery workflows and verification helpers are required' >&2
  exit 1
}

python3 - "$ci" "$release" "$recovery" <<'PY'
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
  grep -Fq "$required" "$ci"
done
for required in \
  '"v0.1.0"' \
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
  grep -Fq "$required" "$release"
done
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
if grep -Eq 'provider-memory-chat:(latest|staging|tmp|temp)' "$release" "$recovery"; then
  echo 'error: mutable OCI tag' >&2
  exit 1
fi
if grep -Eq 'cargo clean|CARGO_TARGET_DIR' "$ci" "$release" "$recovery"; then
  echo 'error: target policy violation' >&2
  exit 1
fi

for required in \
  'workflow_dispatch:' \
  'recover-v0.1.0-from-run-32827819089' \
  'SOURCE_RUN_ID: "32827819089"' \
  'SOURCE_SHA: 564abc55c8e01657ddb0e10938b9f62101e558ae' \
  'SOURCE_TAG_OBJECT: 5bcd812905abe6fd9564be1366b955a9da5a921c' \
  'EXPECTED_SHA: 65f82d6a422b0500269333b79be06c4155d7793df1f80ced12a8b214acb53a6b' \
  './scripts/recover-v0.1.0-artifacts.sh "$RUNNER_TEMP/tagged"' \
  '"$SOURCE_RUN_ID" "$SOURCE_RUN_ATTEMPT"' \
  'git merge-base --is-ancestor "$SOURCE_SHA" refs/remotes/origin/main'; do
  grep -Fq "$required" "$recovery" || {
    echo "error: recovery workflow omits pinned interlock: $required" >&2
    exit 1
  }
done
if grep -Eq 'id-token: write|attestations: write' "$recovery"; then
  echo 'error: recovery must reuse, not replace, immutable tag-run attestations' >&2
  exit 1
fi
if grep -Eq 'cargo (build|test|check)|build-component[.]sh|reproducible-build[.]sh' \
  "$recovery"; then
  echo 'error: recovery workflow must not rebuild the component' >&2
  exit 1
fi
for required in \
  'component_artifact_id=9556257102' \
  'sbom_artifact_id=9556257857' \
  'component_archive_sha=9e9fc0b0d4ea018dc26147d674ed1af9568240044dd12655bf3867003c6633f7' \
  'component_sha=65f82d6a422b0500269333b79be06c4155d7793df1f80ced12a8b214acb53a6b' \
  'source_run_id=32827819089' \
  'cmp "$destination/source-sbom.json" "$destination/attested-sbom.json"'; do
  grep -Fq "$required" "$recovery_helper" || {
    echo "error: recovery helper omits pinned source fact: $required" >&2
    exit 1
  }
done

printf 'workflow full-SHA pins, release/recovery transaction, provenance, and cleanup gates passed\n'
