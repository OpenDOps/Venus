# Convert bench — JS CLI vs Rust `fromDoc`

Filled in [step-rust-adapter](./plan.md#3-step-rust-adapter) (2026-09-13). Pass/fail is **byte identity**, not which is faster.

Same pin: `crates/venus-sidecar/tests/fixtures/large-home.yjs` (2 000 unique `affine:paragraph` + page title). Generator: `apps/web/scripts/write-sidecar-fromdoc-pins.js`.

| Variant | How |
|---|---|
| **JS-from-Rust** | `venus-sidecar` spawns `from-pinned-cli.js` (step 2 product path, **cold spawn counted**) |
| **Pure Rust** | In-process `from_doc::from_pinned_bytes` (no Node) |

Warmup **3**, timed **10**. Profile: `cargo test -p venus-sidecar --test from_doc_bench -- --ignored` (**debug**). p95 is nearest-rank on N=10 (often ≈ max).

| Machine | Pin bytes | Markdown bytes | Blocks | JS median ms | JS p95 | JS max | Rust median ms | Rust p95 | Rust max |
|---|---|---|---|---|---|---|---|---|---|
| macOS Darwin 25.6.0, Apple M4 Max (16 cores); `cargo test` debug (`ARCH=x86_64`, host `uname -m=arm64`) | 1 069 231 | 554 008 | 2001 | 6326.2 | 6991.8 | 6991.8 | 344.3 | 356.7 | 356.7 |
