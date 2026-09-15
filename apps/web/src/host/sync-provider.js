/**
 * Swappable sync seam (M0 step 8).
 *
 * Live hub is VenusHubProvider behind this interface (Yjs binaries + spaces).
 * The editor host must not import the live client — pass a SyncProvider into
 * createM0Workspace instead of changing mount-editor.
 *
 * Default is memory-only: MemoryNoopProvider does not persist or open a socket.
 */
export class MemoryNoopProvider {
  kind = 'memory';
  synced = true;

  connect(_docId, _ydoc) {}

  disconnect(_docId) {}

  whenReady() {
    return Promise.resolve();
  }

  on(event, fn) {
    if (event === 'sync') queueMicrotask(fn);
    return () => {};
  }
}
