# Release

A repository state does not authorize a release. After a reviewed, green validation gate and current clean `main`, the authorized operator confirms the `v<version>` tag, GitHub release (including drafts), and `ghcr.io/dekopon-agents/provider-memory-chat:<version>` are absent. A collision stops release; never overwrite or retag. The operator pushes one annotated tag on `main` matching the Cargo package version. Do not upload locally built bytes.

## Shared workflow

The caller `.github/workflows/release.yml` invokes `dekopon-agents/provider-workflows/.github/workflows/release.yml@main`. Its tag build requires an annotated tag, exact package version and main ancestry; it rebuilds from source with the shared build script, verifies the checksum, runs the test suite, and generates a CycloneDX SBOM. The attestation signer repository is `dekopon-agents/provider-workflows`. The workflow publishes three release assets: `memory-chat-provider.wasm`, `memory-chat-provider.wasm.sha256`, and `memory-chat-provider.cdx.json`. A stable release is marked latest. It publishes the identical Wasm as the sole `application/wasm` layer of `ghcr.io/dekopon-agents/provider-memory-chat:<version>` and verifies that layer's digest against the sidecar.

## Verify the published version

Inspect the release run to completion. Download all three assets outside the checkout. In their directory, check `shasum -a 256 -c memory-chat-provider.wasm.sha256` and validate the CycloneDX JSON. Verify the Wasm provenance against the peeled tag's `main` merge commit:

```console
gh attestation verify memory-chat-provider.wasm -R dekopon-agents/dekopon-provider-memory-chat --format json --signer-repo dekopon-agents/provider-workflows --source-ref refs/tags/v<version> --source-digest <main-merge-SHA>
```

Require the verified subject SHA-256 to equal the sidecar and the peeled tag to equal that merge commit. Use `crane manifest ghcr.io/dekopon-agents/provider-memory-chat:<version>` to assert **exactly one** layer with media type `application/wasm` and digest `sha256:<attested-wasm-SHA256>`. Use `crane digest ghcr.io/dekopon-agents/provider-memory-chat:<version>` for the distinct immutable **manifest** digest to pin deployments, not the Wasm layer digest. Verify the decoded component imports only the SDK's JSONL storage and stdio interfaces, with no WASI or ambient authority.

Release notes must state that broker-owned storage, opaque namespaces and limits are required; record is hidden behind delivered turns; retrieved text is untrusted; and changing provider bytes rotates authority-bound continuity. A synthetic console-smoke scope can test record and reads without accessing real-chat contents.

On workflow failure, stop and inspect the run's exact immutable IDs and artifacts. Do not manually publish, replace bytes, retag, or delete unrelated versions.
