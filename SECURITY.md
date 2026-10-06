# Security policy

## Supported version

Only the newest released version is supported. Report vulnerabilities privately through GitHub Security Advisories for `dekopon-agents/dekopon-provider-memory-chat`; do not include memory content, credentials, namespace keys, or private storage paths in a public issue.

## Security boundary

This component is an untrusted deterministic guest. It imports only `dekopon:storage/jsonl@0.1.1` and `dekopon:stdio/streams@0.1.0`. The broker remains responsible for authentication, authorization, hidden `memory-chat.record` delivery, opaque namespace derivation, single-use grants, storage quotas, Wasmtime limits, audit minimization, and ensuring recording follows one accepted delivery with no retry after uncertainty. The typed SDK uses stdio streams and JSONL imports; `memory-chat.recent` and `memory-chat.search` are explicit reads, while record cannot be proposed from a command. `memory search -` is removed: the provider never reads stdin.

Memory has no encryption-at-rest, deletion/export UI, human-read proof, or delivery proof claim. Retrieved text is untrusted, never an authority or policy input. A synthetic `--smoke-conversation` console session can seed only its confirmed `console-smoke` scope via `:smoke record`; validation needs no real-chat read. Since Dekopon 0.13.0 the storage host applies each call directly and never rolls an invocation back, so a failed record can leave a completed write. Operators must provide broker-owned storage; nothing else can instantiate the component.

## Release trust

Do not trust a tag or upload alone. Verify the release checksum, GitHub build-provenance attestation from `dekopon-agents/provider-workflows`, the released CycloneDX SBOM, exact decoded WIT/import surface, and digest-pinned one-layer OCI bytes. No generated Wasm is source controlled.
