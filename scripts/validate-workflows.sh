#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
python3 - "$root/.github/workflows/ci.yml" "$root/.github/workflows/release.yml" <<'PY'
import pathlib,re,sys
for name in sys.argv[1:]:
    for number,line in enumerate(pathlib.Path(name).read_text().splitlines(),1):
        match=re.match(r"\s*uses:\s*([^\s#]+)",line)
        if match and not re.fullmatch(r"[^@]+@[0-9a-f]{40}",match.group(1)):
            raise SystemExit(f"{name}:{number}: Action is not full-SHA pinned")
PY
ci="$root/.github/workflows/ci.yml"; release="$root/.github/workflows/release.yml"
for required in 'persist-credentials: false' 'Reject tracked Wasm' './scripts/validate-source.sh' './scripts/reproducible-build.sh'; do grep -Fq "$required" "$ci"; done
for required in '"v0.1.0"' 'git cat-file -t' 'subject-path: dist/memory-chat-provider.wasm' 'predicate-type: https://cyclonedx.org/bom' 'memory-chat-provider.wasm:application/wasm' 'application/vnd.dekopon.provider.v1+wasm' 'provider-memory-chat/versions' 'draft: false' 'make_latest: "false"' 'cleanup_failed_release:' 'cleanup-release-id' 'cleanup-release-tag' 'cleanup-package' 'failed cleanup did not restore original release/package absence'; do grep -Fq "$required" "$release"; done
if ! sed -n '/^  build:/,/^  attest:/p' "$release" | grep -Eq 'test .*git rev-parse refs/remotes/origin/main.* = .*GITHUB_SHA'; then echo 'error: release preflight does not require current main identity' >&2; exit 1; fi
if grep -Fq 'git merge-base --is-ancestor' "$release"; then echo 'error: release tag need not equal current main' >&2; exit 1; fi
if sed -n '/^  draft:/,/^  ghcr:/p' "$release" | grep -Fq 'id-token: write'; then echo 'error: draft job has unnecessary OIDC authority' >&2; exit 1; fi
if grep -Eq 'provider-memory-chat:(latest|staging|tmp|temp)' "$release"; then echo 'error: mutable OCI tag' >&2; exit 1; fi
if grep -Eq 'cargo clean|CARGO_TARGET_DIR' "$ci" "$release"; then echo 'error: target policy violation' >&2; exit 1; fi
printf 'workflow full-SHA pins, least privileges, immutable transaction, provenance, and cleanup gates passed\n'
