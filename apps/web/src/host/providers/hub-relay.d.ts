import type { HubWorkerStats } from './shared-worker-socket.js';

export interface RelayPort {
  postMessage(msg: unknown, transfer?: Transferable[]): void;
}

export function readTabFrame(bytes: Uint8Array): {
  step1: number;
  updates: Uint8Array[];
};

export function readHubFrame(
  bytes: Uint8Array,
):
  | { kind: 'step1' | 'update' | 'other' }
  | { kind: 'step2'; update: Uint8Array };

export class HubRelay {
  constructor(createSocket: (url: string) => WebSocket);
  attach(port: RelayPort): void;
  handle(port: RelayPort, msg: unknown): void;
  detachPort(port: RelayPort): void;
  stats(): HubWorkerStats;
}
