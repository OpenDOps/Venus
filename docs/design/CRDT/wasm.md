# No Rust in the editor

**Status:** locked. BlockSuite (Lit + `yjs@13.6.32` on `store.spaceDoc`) is the client editor. Venus does **not** inject Rust, WASM, y-octo, or a second CRDT into the tab or WebView.

Rust is **only**:

1. **Hub merge** — apply / hydrate / compact, broadcast, persist ([M3.0 HA](../M3.0/high-availability.md)).
2. **`toDoc` / pin convert** — worker beside the hub: y-octo hydrate of the pin, then M2 `MarkdownAdapter` (`from-doc.js` / slice `toDoc`).

Spectator pane `fromDoc` stays JS. Native desktop/mobile later: **same** BlockSuite in a WebView, **native** hub process + SQLite. Not a Rust editor. Not WASM-as-hub.

## Do not

- WASM `Y.Doc` (or dual-write WASM + JS) behind `SyncProvider`.
- `@affine/native` or a Venus NAPI that replaces `spaceDoc`.
- Canvas / Skia / WASM VDOM as the page editor.
- `wasm32` as an M3.0 (or product-client) target.

Why a WASM/Rust editor does not pay: Lit already patches dirty DOM; the browser owns caret/IME; FFI per keystroke is extra. Analysis that led here is superseded by this lock.

Hub y-octo must still emit update v1 that JS Yjs round-trips as `Y.Text` / `Y.Array` ([AFFiNE#14275](https://github.com/toeverything/AFFiNE/issues/14275)).
