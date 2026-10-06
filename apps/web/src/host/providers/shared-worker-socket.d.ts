export interface HubWorkerStats {
  /** Tabs (MessagePorts) attached to the worker. */
  ports: number;
  sockets: Array<{
    url: string;
    readyState: number;
    /** Provider sessions on this socket, across tabs. */
    channels: number;
    /** Tabs with a session on this socket. */
    ports: number;
  }>;
}

export const HUB_WORKER_NAME_PREFIX: string;

export class SharedWorkerSockets {
  readonly kind: 'shared-worker';
  failed: boolean;
  constructor(
    port: MessagePort,
    worker?: { addEventListener?: (type: string, fn: () => void) => void } | null,
  );
  open(url: string): WebSocket;
  stats(): Promise<HubWorkerStats>;
}

export function createSharedWorkerSockets(
  hubUrl: string,
): SharedWorkerSockets | null;
