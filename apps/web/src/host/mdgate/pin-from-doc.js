import * as Y from 'yjs';
import { hydrateM0FromUpdate } from '../workspace.js';
import { encodeSidecarClock, fromDoc } from './from-doc.js';

/**
 * Copy Yjs update v1 + state-vector clock. Live Store may keep mutating.
 * This is the pin. Markdown is not pinned.
 */
export function pinYjsBytes(spaceDoc) {
  return {
    bytes: Y.encodeStateAsUpdate(spaceDoc),
    clock: encodeSidecarClock(spaceDoc),
  };
}

/**
 * Full `fromDoc` on an offline clone of `bytes`. Never incremental RAM
 * export. Never `fromDoc` the live published Store. Sidecar clock is the
 * pin clock.
 */
export async function fromPinnedBytes(bytes, options = {}) {
  const session = hydrateM0FromUpdate(bytes, {
    blobSources: options.blobSources,
  });
  const exported = await fromDoc(session.store, session.workspace);
  const clock = options.clock ?? exported.sidecar.clock;
  const docId = options.docId ?? exported.sidecar.docId;
  return {
    markdown: exported.markdown,
    sidecar: {
      ...exported.sidecar,
      docId,
      clock,
    },
  };
}

/**
 * Path B / git convert: pin bytes first, then markdown + sidecar from that
 * pin. Do not pass `incrementalFromDoc` output here.
 */
export async function pinThenFromDoc(store, _workspace, options = {}) {
  const pin = pinYjsBytes(store.spaceDoc);
  const blobSources =
    options.blobSources ??
    (store.blobSync?.main ? { main: store.blobSync.main } : undefined);
  const converted = await fromPinnedBytes(pin.bytes, {
    clock: pin.clock,
    docId: store.doc?.id ?? store.id,
    blobSources,
  });
  return {
    pin,
    markdown: converted.markdown,
    sidecar: converted.sidecar,
  };
}
