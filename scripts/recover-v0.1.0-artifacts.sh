#!/usr/bin/env bash
# Download and verify only artifacts produced by immutable tag release run 32827819089.
# This one-off helper never builds or substitutes provider bytes.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
destination=${1:?usage: recover-v0.1.0-artifacts.sh DESTINATION}

repo=dekopon-agents/dekopon-provider-memory-chat
source_run_id=32827819089
source_run_attempt=1
source_workflow_id=341898482
source_sha=564abc55c8e01657ddb0e10938b9f62101e558ae
tag=v0.1.0
tag_object=5bcd812905abe6fd9564be1366b955a9da5a921c
source_build_job_id=97739636135
source_attest_job_id=97745520570
source_draft_job_id=97745594853
source_cleanup_job_id=97746169711
component_artifact_id=9556257102
component_archive_sha=9e9fc0b0d4ea018dc26147d674ed1af9568240044dd12655bf3867003c6633f7
component_archive_size=94744
sbom_artifact_id=9556257857
sbom_archive_sha=f587516f9473e5e1a1b2cdc70abf1b71227ffcaab9cb666de30279b1278d908d
sbom_archive_size=6626
component_sha=65f82d6a422b0500269333b79be06c4155d7793df1f80ced12a8b214acb53a6b
checksum_sha=1bc71aef8ceac3f8864fc34db8ae2ae3513098274791225f04268dbd47b9c8b7
sbom_sha=0608a6503400bd57e4c24f3ec6f0d0b812ad4be0e8acf1f831f1d579e1d72234

: "${GH_TOKEN:?GH_TOKEN is required to read captured Actions artifacts}"
[[ "${GITHUB_REPOSITORY:-$repo}" == "$repo" ]]
for command in base64 curl gh git jq shasum unzip; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "error: $command is required" >&2
    exit 1
  }
done

run=$(gh api "repos/$repo/actions/runs/$source_run_id")
jq -e \
  --arg repo "$repo" --arg sha "$source_sha" \
  --argjson id "$source_run_id" --argjson workflow_id "$source_workflow_id" '
    .id == $id and .workflow_id == $workflow_id and
    .name == "Immutable v0.1.0 release" and .path == ".github/workflows/release.yml" and
    .event == "push" and .status == "completed" and .conclusion == "failure" and
    .head_branch == "v0.1.0" and .head_sha == $sha and .run_attempt == 1 and
    .repository.full_name == $repo
  ' <<<"$run" >/dev/null
jobs=$(gh api "repos/$repo/actions/runs/$source_run_id/jobs?filter=all&per_page=100")
jq -e \
  --arg cleanup_name "remove only this failed run's proven release and sole final manifest" \
  --argjson build "$source_build_job_id" --argjson attest "$source_attest_job_id" \
  --argjson draft "$source_draft_job_id" --argjson cleanup "$source_cleanup_job_id" '
    .total_count == 7 and
    any(.jobs[]; .id == $build and
      .name == "verify immutable source and exact component" and .conclusion == "success") and
    any(.jobs[]; .id == $attest and
      .name == "attest component and checksum" and .conclusion == "success") and
    any(.jobs[]; .id == $draft and
      .name == "create and verify run-owned exact draft" and .conclusion == "failure") and
    any(.jobs[]; .id == $cleanup and
      .name == $cleanup_name and .conclusion == "success") and
    ([.jobs[] | select(
      .id != $build and .id != $attest and .id != $draft and .id != $cleanup
    ) | .conclusion] | all(. == "skipped"))
  ' <<<"$jobs" >/dev/null
jq -e --argjson draft "$source_draft_job_id" '
  [.jobs[] | select(.id == $draft)][0] as $job |
  ([$job.steps[] | select(
    .name == "Create a run-marked draft with exactly two assets"
  ) | .conclusion] == ["failure"]) and
  ([$job.steps[] | select(
    .name == "Redownload both draft assets by ID and compare bytes"
  ) | .conclusion] == ["skipped"])
' <<<"$jobs" >/dev/null

artifacts=$(gh api "repos/$repo/actions/runs/$source_run_id/artifacts?per_page=100")
jq -e \
  --arg component_digest "sha256:$component_archive_sha" \
  --arg sbom_digest "sha256:$sbom_archive_sha" \
  --arg sha "$source_sha" \
  --argjson component_id "$component_artifact_id" \
  --argjson component_size "$component_archive_size" \
  --argjson run_id "$source_run_id" \
  --argjson sbom_id "$sbom_artifact_id" \
  --argjson sbom_size "$sbom_archive_size" '
    .total_count == 2 and
    ([.artifacts[] | {
      id, name, size_in_bytes, expired, digest,
      run_id: .workflow_run.id,
      head_branch: .workflow_run.head_branch,
      head_sha: .workflow_run.head_sha
    }] | sort_by(.id)) == ([
      {
        id: $component_id, name: "release-assets", size_in_bytes: $component_size,
        expired: false, digest: $component_digest, run_id: $run_id,
        head_branch: "v0.1.0", head_sha: $sha
      },
      {
        id: $sbom_id, name: "release-sbom", size_in_bytes: $sbom_size,
        expired: false, digest: $sbom_digest, run_id: $run_id,
        head_branch: "v0.1.0", head_sha: $sha
      }
    ] | sort_by(.id))
  ' <<<"$artifacts" >/dev/null

git -C "$root" fetch --force origin "refs/tags/$tag:refs/tags/$tag"
[[ "$(git -C "$root" cat-file -t "refs/tags/$tag")" == tag ]]
[[ "$(git -C "$root" rev-parse "refs/tags/$tag")" == "$tag_object" ]]
[[ "$(git -C "$root" rev-parse "refs/tags/$tag^{}")" == "$source_sha" ]]
git -C "$root" merge-base --is-ancestor "$source_sha" HEAD

rm -rf "$destination"
mkdir -p "$destination/component" "$destination/sbom" "$destination/archives"
download() {
  local artifact_id=$1
  local archive_sha=$2
  local output=$3
  gh api "repos/$repo/actions/artifacts/$artifact_id/zip" >"$output"
  [[ "$(shasum -a 256 "$output" | awk '{print $1}')" == "$archive_sha" ]]
}
download "$component_artifact_id" "$component_archive_sha" \
  "$destination/archives/release-assets.zip"
download "$sbom_artifact_id" "$sbom_archive_sha" \
  "$destination/archives/release-sbom.zip"
unzip -q "$destination/archives/release-assets.zip" -d "$destination/component"
unzip -q "$destination/archives/release-sbom.zip" -d "$destination/sbom"

component_files=$(cd "$destination/component" && find . -type f -print | LC_ALL=C sort)
sbom_files=$(cd "$destination/sbom" && find . -type f -print | LC_ALL=C sort)
[[ "$component_files" == $'./memory-chat-provider.wasm\n./memory-chat-provider.wasm.sha256' ]]
[[ "$sbom_files" == './memory-chat-provider.cdx.json' ]]
[[ "$(shasum -a 256 "$destination/component/memory-chat-provider.wasm" |
  awk '{print $1}')" == "$component_sha" ]]
[[ "$(shasum -a 256 "$destination/component/memory-chat-provider.wasm.sha256" |
  awk '{print $1}')" == "$checksum_sha" ]]
[[ "$(shasum -a 256 "$destination/sbom/memory-chat-provider.cdx.json" |
  awk '{print $1}')" == "$sbom_sha" ]]
[[ "$(cat "$destination/component/memory-chat-provider.wasm.sha256")" == \
   "$component_sha  memory-chat-provider.wasm" ]]
(cd "$destination/component" && shasum -a 256 -c memory-chat-provider.wasm.sha256)
[[ "$(wc -c <"$destination/component/memory-chat-provider.wasm" |
  tr -d '[:space:]')" == 248638 ]]
jq -e '
  .bomFormat == "CycloneDX" and .specVersion == "1.5" and
  .metadata.component.name == "dekopon-memory-chat-provider"
' "$destination/sbom/memory-chat-provider.cdx.json" >/dev/null

for predicate in 'https://slsa.dev/provenance/v1' 'https://cyclonedx.org/bom'; do
  "$root/scripts/verify-attestation-anonymously.sh" \
    "$destination/component/memory-chat-provider.wasm" "$repo" "$component_sha" \
    "$predicate" "$repo/.github/workflows/release.yml" "refs/tags/$tag" \
    "$source_sha" "$source_run_id" "$source_run_attempt"
done
curl --fail --silent --show-error --location --get \
  --data-urlencode 'predicate_type=https://cyclonedx.org/bom' \
  "https://api.github.com/repos/$repo/attestations/sha256:$component_sha" \
  >"$destination/sbom-attestations.json"
jq -e '.attestations | length == 1' "$destination/sbom-attestations.json" >/dev/null
jq -r '.attestations[0].bundle.dsseEnvelope.payload' \
  "$destination/sbom-attestations.json" | base64 --decode \
  >"$destination/sbom-statement.json"
jq -S '.predicate' "$destination/sbom-statement.json" >"$destination/attested-sbom.json"
jq -S '.' "$destination/sbom/memory-chat-provider.cdx.json" >"$destination/source-sbom.json"
cmp "$destination/source-sbom.json" "$destination/attested-sbom.json"

printf 'verified immutable tag-run artifacts: run=%s component=sha256:%s sbom=sha256:%s\n' \
  "$source_run_id" "$component_sha" "$sbom_sha"
