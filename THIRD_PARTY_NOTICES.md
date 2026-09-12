# Third-party notices

`dekopon-memory-chat-provider` project source is licensed under **MIT OR Apache-2.0**. The release component statically links permissively licensed Rust dependencies. `Cargo.lock` is the exact transitive-version and crates.io-checksum authority; `cargo deny check licenses advisories bans sources` enforces the source and license policy.

The component uses exact crates.io releases `dekopon-provider-sdk 0.13.0` and `dekopon-provider-storage 0.13.0`, plus `serde 1.0.229`, `serde_json 1.0.151`, and `wit-bindgen 0.62.0`. Their transitive crates are licensed under the allowlist in `deny.toml`, including MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, Unicode-3.0, Unlicense, and Zlib. Native-only test dependencies, including `dekopon-provider-sdk-testkit 0.13.0`, Tokio, Wasmtime, broker-host, and storage-host, are not linked into the Wasm component.

The complete MIT and Apache-2.0 project license texts are in `LICENSE-MIT` and `LICENSE-APACHE`. Reproduce the inventory with:

```console
cargo deny list
cargo deny check licenses advisories bans sources
cargo tree --locked --target wasm32-unknown-unknown --edges normal,build
```
