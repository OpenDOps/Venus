/** Direct keck (Vite / `pnpm test:e2e:m1`). Compose web uses same-origin :8080. */
export const KECK_DIRECT_WS = 'ws://127.0.0.1:3000/collaboration/venus-m0';

export function expectedCollaborationWs(): string {
  const base = process.env.PLAYWRIGHT_BASE_URL;
  if (!base) return KECK_DIRECT_WS;
  const u = new URL(base);
  const proto = u.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${u.host}/collaboration/venus-m0`;
}
