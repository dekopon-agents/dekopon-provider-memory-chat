# Dekopon memory-chat provider

A standalone broker-only WebAssembly component implementing hidden `memory.chat.record` and explicit `memory.chat.recent` and `memory.chat.search`. The component imports exactly `dekopon:storage/jsonl@0.1.0`; it has no HTTP, WASI, filesystem, environment, clock, random, subprocess, or other ambient authority.

## Authority and privacy boundary

The provider owns JSON encoding, bounded retrieval, literal Unicode-lowercase search, deduplication, and compaction for the logical names `turns.jsonl` and `dedup.jsonl`. It does **not** own authorization, authenticated chat identity, namespaces, grants, quotas, retention policy, or storage paths. Those remain broker/storage-host responsibilities.

`memory.chat.record` is intentionally absent from command resolution and its advertised schema is an empty closed object. Only the broker's hidden post-acceptance route may supply its curated record fields. Retrieved text is untrusted content: it is never identity, policy input, or automatically replayed prompt context. Storage telemetry must not expose turn content or identifying namespace material.

The storage host must bind each invocation to a broker-authorized JSONL grant and opaque chat namespace. As of Dekopon 0.13.0 each guest `append` and `replace` is applied by the host when it is called; there is no invocation transaction and no rollback. A record whose dedup append is refused keeps the turn append that already completed, and the provider reports the refusal rather than pretending the write did not happen.

## Exact capabilities

| Capability | Effect | Risk |
|---|---|---|
| `memory.chat.record` | local-write | Medium |
| `memory.chat.recent` | read-only | High |
| `memory.chat.search` | read-only | High |

The only command word is `memory`. It behaves like a command-line program through the `run-command` export: `memory --help` renders a help page on standard output at status 0, `memory recent --last N` and `memory search --query TEXT` propose their capability, `memory search -` takes the query from the value piped into the word, and any other argv renders a usage error on standard error at status 2. Record never runs from a command.

## Durable format and bounds

`turns.jsonl` contains strict LF-terminated `{format:"dekopon.chat-memory.turn",version:1,id,commitment,user,assistant}` records. It compacts with hysteresis at `compactionThresholdBytes`, keeping the newest bounded lookback that fits `compactionTargetBytes`. `dedup.jsonl` contains strict LF-terminated `{format:"dekopon.chat-memory.dedup",version:1,id,commitment}` entries; it is finite and never compacted.

Unknown fields, wrong format/version, malformed JSON, and truncated final lines fail as `memory-corrupt`. Duplicate ID and commitment succeeds without mutation; a changed commitment is `dedup-conflict`; record/byte exhaustion is `dedup-capacity` while reads remain available. Recent and search return whole chronological turns within `maxResultBytes`, or `result-too-large`. Search is literal substring matching after Unicode lowercase conversion, not regex or semantic search.

All operational ceilings (`maxTurnBytes`, lookback/result/dedup limits, and compaction target/threshold) are broker-curated inputs, not model-selected controls. Storage reads use 256 KiB chunks. The broker must additionally enforce storage, call, memory, fuel, timeout, and output ceilings.

## Run and deployment

Nothing outside a broker can supply the JSONL import: an empty Wasmtime linker refuses to instantiate the component. Install it only in a Dekopon 0.18.0 broker with broker-owned JSONL storage, namespace derivation, and limits. Authority-bound continuity intentionally rotates when provider bytes or effective authority change; explicit stable continuity preserves addressing while each operation is still freshly authorized.

Each release puts exactly `memory-chat-provider.wasm` and its `.sha256` on GitHub. Identical Wasm bytes are the sole `application/wasm` layer at `ghcr.io/dekopon-agents/provider-memory-chat:<version>`; no `latest` tag is published.

## Build and validation

Generated Wasm is ignored and must never be committed. Each checkout uses its ordinary `target/` and the machine's configured global compiler cache.

Build the component with the shared [`provider-workflows`](https://github.com/dekopon-agents/provider-workflows) build script, run from a sibling checkout:

```console
../provider-workflows/build.sh
```

Formatting, lints, `cargo deny`, the reproducible component build, and the test suite are all gated by the shared `ci / validate` workflow rather than local scripts. Tests that load the built component read its path from the `DEKOPON_PROVIDER_COMPONENT` environment variable.

See [SECURITY.md](SECURITY.md) and [RELEASE.md](RELEASE.md).

## License

Project-authored source is licensed under MIT OR Apache-2.0.
