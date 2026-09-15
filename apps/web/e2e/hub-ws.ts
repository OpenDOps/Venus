/** Direct hub (Vite / `pnpm test:e2e:m1`). Compose web uses same-origin :8080. */
import { COLLABORATION_PATH, WORKSPACE_ID } from '../src/host/ids.js';

export { WORKSPACE_ID };
export const HUB_DIRECT_WS = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;

export function expectedCollaborationWs(): string {
  const base = process.env.PLAYWRIGHT_BASE_URL;
  if (!base) return HUB_DIRECT_WS;
  const u = new URL(base);
  const proto = u.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${u.host}${COLLABORATION_PATH}`;
}

/** Fail if nothing listens, or if `:3000` is keck rather than `venus-hub`. */
export async function assertHubOn3000(): Promise<void> {
  let body = '';
  try {
    const res = await fetch('http://127.0.0.1:3000/', {
      signal: AbortSignal.timeout(3000),
    });
    body = (await res.text()).trim();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (/ECONNREFUSED|fetch failed|AbortError|TimeoutError/i.test(msg)) {
      throw new Error(
        `hub is not up on :3000 (${msg}). Start with pnpm sync:up from the repo root.`,
      );
    }
    throw err;
  }
  if (body !== 'venus-hub') {
    throw new Error(
      `:3000 is not the Venus hub (GET / → ${JSON.stringify(body)}). Fail if keck is the process.`,
    );
  }
}

/** Fail if the snapshotter is not listening (Flush / wiki write). */
export async function assertSidecarOn3002(): Promise<void> {
  let body = '';
  try {
    const res = await fetch('http://127.0.0.1:3002/', {
      signal: AbortSignal.timeout(3000),
    });
    body = (await res.text()).trim();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (/ECONNREFUSED|fetch failed|AbortError|TimeoutError/i.test(msg)) {
      throw new Error(
        `sidecar is not up on :3002 (${msg}). Start with docker compose --profile snapshot up sidecar (WIKI_DIR bind-mounted) or cargo run -p venus-sidecar with DATABASE_URL and WIKI_DIR=wiki.`,
      );
    }
    throw err;
  }
  if (body !== 'venus-sidecar') {
    throw new Error(
      `:3002 is not venus-sidecar (GET / → ${JSON.stringify(body)}).`,
    );
  }
}
