# Immutable v0.1.0 release

No release is authorized by this repository state. A human may authorize **only v0.1.0** after reviewing a clean current `main`, the complete validation gate, and two independent archive builds.

## Preflight

1. Confirm `main` is clean/current and CI succeeds.
2. Run `./scripts/validate.sh` and `./scripts/reproducible-build.sh`.
3. Prove tag `v0.1.0`, every release with that tag, and the complete `ghcr.io/dekopon-agents/provider-memory-chat` package are absent. Stop rather than overwrite any state.
4. Create and push one annotated `v0.1.0` tag at current `main`. Never upload locally built bytes.

## Actions-owned transaction

The tag workflow rejects another tag, a lightweight tag, version/ref/SHA mismatch, or a tag not on current `main`. Actions rebuilds from immutable source and creates exactly `memory-chat-provider.wasm` and `memory-chat-provider.wasm.sha256`. The SBOM is an attestation predicate, not a release asset.

Actions attests the component with build provenance and CycloneDX, then creates one run-marker-owned draft and captures its immutable ID. It uploads/redownloads the two assets by ID and byte-compares them. It pushes the same Wasm as one `application/wasm` layer under artifact type `application/vnd.dekopon.provider.v1+wasm` at only tag `0.1.0`, verifies package/version/tag cardinality and anonymously pulled bytes, and makes the package public only after verification.

Immediately before the final mutation, Actions rechecks tag/main, draft asset IDs and bytes, exact JSONL-only WIT with no WASI, public attestations, digest-pinned anonymous OCI bytes, and sole package version/tag. Only then does it publish the captured release with `draft=false`, `prerelease=false`, and `make_latest=false`; credentials-free verification follows.

Release notes must state that broker-owned storage, opaque namespaces, limits, and invocation transactions are required; direct runner cannot load the component; record is hidden; retrieved text is untrusted; and changing provider bytes rotates authority-bound continuity.

## Failure and recovery

Failure/cancellation cleanup may mutate only state proven to belong to the exact run using captured release/package IDs, digest, and marker. Hide/delete public OCI bytes first, then delete the captured draft (or run-owned not-yet-accepted final release), and prove original absence. Never infer ownership from a mutable tag; never delete unrelated package versions; surface cleanup failures. Attestations may remain as immutable evidence.

If cleanup cannot prove ownership or original absence, stop. A recovery workflow must be written for the observed immutable IDs and reviewed as a separate commit; it must reuse the retained successful Actions artifact and never rebuild/substitute bytes.
