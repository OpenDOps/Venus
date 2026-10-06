/**
 * SharedWorker script (one per origin + hub URL). Holds the wire A sockets
 * for every tab of this browsing profile. `Y.Doc` and BlockSuite stay in
 * the tab; this script only relays bytes. Not a Service Worker.
 */
import { HubRelay } from './hub-relay.js';

const relay = new HubRelay((url) => new WebSocket(url, ['AFFiNE']));

/** @param {MessageEvent} event */
self.addEventListener('connect', (event) => {
  const port = event.ports[0];
  if (!port) return;
  port.addEventListener('message', (ev) => relay.handle(port, ev.data));
  port.addEventListener('close', () => relay.detachPort(port));
  port.start();
  relay.attach(port);
});
