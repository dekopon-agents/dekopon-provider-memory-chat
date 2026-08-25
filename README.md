# Dekopon memory-chat provider

A standalone broker-only WebAssembly component implementing hidden `memory.chat.record` and explicit `memory.chat.recent` and `memory.chat.search`. The component imports exactly `dekopon:storage/jsonl@0.1.0`; it has no HTTP, WASI, filesystem, environment, clock, random, subprocess, or other ambient authority.

## Authority and privacy boundary

The provider owns JSON encoding, bounded retrieval, literal Unicode-lowercase search, deduplication, and compaction for the logical names `turns.jsonl` and `dedup.jsonl`. It does **not** own authorization, authenticated chat identity, namespaces, grants, quotas, transaction commit, retention policy, or storage paths. Those remain broker/storage-host responsibilities.

`memory.chat.record` is intentionally absent from command resolution and its advertised schema is an empty closed object. Only the broker's hidden post-acceptance route may supply its curated record fields. Retrieved text is untrusted content: it is never identity, policy input, or automatically replayed prompt context. Storage telemetry must not expose turn content or identifying namespace material.

The storage host must bind each invocation to a broker-authorized JSONL grant and opaque chat namespace. Guest `append` and `replace` calls are provisional operations in one invocation transaction: the host commits them only after a valid successful provider response and rolls all of them back on failure. Individual WIT calls are not durability boundaries.

## Exact capabilities

| Capability | Effect | Risk | Idempotency |
|---|---|---|---|
| `memory.chat.record` | local-write | Medium | conditional |
| `memory.chat.recent` | read-only | High | idempotent |
| `memory.chat.search` | read-only | High | idempotent |

The only command word is `memory`, resolving `memory recent --last N` and `memory search --query TEXT`. Record never resolves from a command.

## Durable format and bounds

`turns.jsonl` contains strict LF-terminated `{format:"dekopon.chat-memory.turn",version:1,id,commitment,user,assistant}` records. It compacts with hysteresis at `compactionThresholdBytes`, keeping the newest bounded lookback that fits `compactionTargetBytes`. `dedup.jsonl` contains strict LF-terminated `{format:"dekopon.chat-memory.dedup",version:1,id,commitment}` entries; it is finite and never compacted.

Unknown fields, wrong format/version, malformed JSON, and truncated final lines fail as `memory-corrupt`. Duplicate ID and commitment succeeds without mutation; a changed commitment is `dedup-conflict`; record/byte exhaustion is `dedup-capacity` while reads remain available. Recent and search return whole chronological turns within `maxResultBytes`, or `result-too-large`. Search is literal substring matching after Unicode lowercase conversion, not regex or semantic search.

All operational ceilings (`maxTurnBytes`, lookback/result/dedup limits, and compaction target/threshold) are broker-curated inputs, not model-selected controls. Storage reads use 256 KiB chunks. The broker must additionally enforce storage, call, memory, fuel, timeout, and output ceilings.

## Run and deployment

Direct `dekopon-run invoke` cannot load this imported component and must reject it. Install it only in a Dekopon 0.11.1 broker with broker-owned JSONL storage, namespace derivation, limits, and transaction support. Authority-bound continuity intentionally rotates when provider bytes or effective authority change; explicit stable continuity preserves addressing while each operation is still freshly authorized.

When v0.1.0 is released, GitHub contains exactly `memory-chat-provider.wasm` and its `.sha256`. Identical Wasm bytes are the sole `application/wasm` layer at `ghcr.io/dekopon-agents/provider-memory-chat:0.1.0`; no `latest` tag is published.

## Build and validation

Generated Wasm is ignored and must never be committed. Each checkout uses its ordinary `target/` and the machine's configured global compiler cache.

```console
rustup toolchain install 1.89.0 --profile minimal
rustup toolchain install 1.97.0 --profile minimal --component clippy --component rustfmt
rustup target add wasm32-unknown-unknown --toolchain 1.97.0
cargo +1.97.0 install wasm-tools --version 1.236.1 --locked
./scripts/validate.sh
./scripts/reproducible-build.sh
```

See [SECURITY.md](SECURITY.md), [RELEASE.md](RELEASE.md), and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

Project-authored source is licensed under MIT OR Apache-2.0.
