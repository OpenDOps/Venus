/** Direct hub (Vite / `pnpm test:e2e:m1`). Compose web uses same-origin :8080. */
import { COLLABORATION_PATH, WORKSPACE_ID } from '../src/host/ids.js';

export { WORKSPACE_ID };
export const KECK_DIRECT_WS = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;

export function expectedCollaborationWs(): string {
  const base = process.env.PLAYWRIGHT_BASE_URL;
  if (!base) return KECK_DIRECT_WS;
  const u = new URL(base);
  const proto = u.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${u.host}${COLLABORATION_PATH}`;
}
