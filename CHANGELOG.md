# Changelog

## 0.3.0 - unreleased

- Move to the `dekopon-provider-sdk` 0.15.0 line: bump `dekopon-provider-sdk`, `dekopon-provider-storage`, and `dekopon-provider-sdk-testkit` from `=0.13.0` to `=0.15.0`. The caller-owned WIT (`dekopon:provider@0.3.0`, `dekopon:storage`) is byte-identical between the two SDK releases, so no WIT or generated-binding change was needed.
- No source change was needed to adopt the 0.15.0 API: this provider never constructs `CommandInvocation` directly (it goes through `CommandRun::proposal`), and its `tests/broker.rs` reads `CommandRunOutcome` back only through `serde_json::to_value`, which is forward-compatible with the new `secretUse` field being omitted when absent.
- Delete the stale one-shot `.github/workflows/recover-v0.1.0.yml` and the `scripts/validate-workflows.sh` checks that pinned its exact content. `scripts/recover-v0.1.0-artifacts.sh` is now unreferenced and left in place.
- Fix the hardcoded `0.13.0` version pins in `scripts/check-dependencies.sh` and `scripts/generate-sbom.sh` (both now assert `0.15.0`).

## 0.2.0 - unreleased

- Move to the `dekopon-provider-sdk` 0.13.0 line: `dekopon:provider@0.3.0` WIT, Rust 1.98.1, wit-bindgen 0.62.0, wasm-tools 1.259.0, and Wasmtime 48.0.2.
- Replace `resolve-command` with the `provider-cli` world's `run-command`. `memory --help` renders a help page on stdout at status 0, an argv the word does not accept renders a usage error on stderr at status 2, and `memory search -` takes its query from the value piped into the word. `memory recent --last N` and `memory search --query TEXT` propose exactly what they proposed before. `resolve-command` is gone, not kept alongside.
- Drop `idempotency` from every capability: the SDK removed the field.
- Record the loss of invocation rollback. Dekopon 0.13.0 applies each storage call when the guest makes it, so a record whose dedup append is refused keeps the turn append that already completed. The provider still fails closed on the operation; the durability claim in the README, SECURITY, and release notes was wrong and is corrected.
- Delete the `dekopon-run` gates from CI, release, and `scripts/test-direct-refusal.sh`. It is retired at 0.11.1 and would reject a 0.13.0 manifest. The empty-Wasmtime-linker refusal stays, and the broker, storage, and command-word ground it covered is now driven through the testkit's `FakeBroker` in `tests/broker.rs`.
- Generalize `release.yml` from the one-shot v0.1.0 transaction: it triggers on `v*`, derives the version from the crate, and its absence proofs and failure cleanup act on this run's release and package version instead of the whole package.

## 0.1.0 - 2026-08-25

- Extract the durable JSONL chat-memory provider from Dekopon 0.11.1 without changing its capabilities, bounds, or privacy boundary.
- Own and verify the exact provider and storage WIT inputs.
- Add independent native, component, broker/storage, resource, license, and reproducibility gates.
