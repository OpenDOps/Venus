import { expect, test } from 'vitest';
import {
  OctoBaseBlobSource,
  blobOriginFromSyncUrl,
} from './blob-source.js';
import { blobSourcesFromEnv } from './from-env.js';

function urlOf(input: RequestInfo | URL) {
  if (typeof input === 'string') return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

test('blobOriginFromSyncUrl maps ws to http origin', () => {
  expect(
    blobOriginFromSyncUrl('ws://127.0.0.1:3000/collaboration/venus-m0'),
  ).toBe('http://127.0.0.1:3000');
  expect(
    blobOriginFromSyncUrl('ws://127.0.0.1:3000/collaboration/venus-m0', {
      sameOrigin: true,
    }),
  ).toBe('');
});

test('blobSourcesFromEnv is unset without VITE_SYNC_URL', () => {
  expect(blobSourcesFromEnv({})).toBeUndefined();
  expect(blobSourcesFromEnv({ VITE_SYNC_URL: '  ' })).toBeUndefined();
});

test('blobSourcesFromEnv uses OctoBaseBlobSource when VITE_SYNC_URL is set', () => {
  const sources = blobSourcesFromEnv({
    VITE_SYNC_URL: 'ws://127.0.0.1:3000/collaboration/venus-m0',
  });
  expect(sources?.main).toBeInstanceOf(OctoBaseBlobSource);
  expect(sources?.main.name).toBe('octobase');
});

test('blobSourcesFromEnv same-origin uses empty origin (nginx /api proxy)', () => {
  const sources = blobSourcesFromEnv({ VITE_SYNC_URL: 'same-origin' });
  expect(sources?.main).toBeInstanceOf(OctoBaseBlobSource);
  expect(sources?.main.origin).toBe('');
});

test('OctoBaseBlobSource POSTs bytes, GETs them, list is empty', async () => {
  const stored = new Map<string, ArrayBuffer>();
  const orig = globalThis.fetch;
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = urlOf(input);
    const method = (init?.method ?? 'GET').toUpperCase();
    if (method === 'POST' && url.endsWith('/api/blobs/venus-m0')) {
      const body = init?.body;
      const buf =
        body instanceof ArrayBuffer
          ? body
          : body instanceof Uint8Array
            ? body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength)
            : new ArrayBuffer(0);
      stored.set('posted', buf as ArrayBuffer);
      return new Response(JSON.stringify({ id: 'abc', exists: true }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    }
    if (method === 'GET' && url.includes('/api/blobs/venus-m0/')) {
      const key = url.split('/').pop() ?? '';
      if (key === 'missing') {
        return new Response(null, { status: 404 });
      }
      const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
      return new Response(png, {
        status: 200,
        headers: { 'Content-Type': 'application/octet-stream' },
      });
    }
    if (method === 'DELETE') {
      return new Response(null, { status: 204 });
    }
    return new Response('nope', { status: 500 });
  }) as typeof fetch;

  try {
    const src = new OctoBaseBlobSource({
      workspaceId: 'venus-m0',
      origin: 'http://127.0.0.1:3000',
    });
    expect(await src.list()).toEqual([]);
    expect(await src.get('missing')).toBeNull();

    const key = await src.set(
      'should-keep-this-key',
      new Blob([new Uint8Array([1, 2, 3])]),
    );
    expect(key).toBe('should-keep-this-key');
    expect(stored.get('posted')?.byteLength).toBe(3);

    const got = await src.get('any');
    expect(got).toBeInstanceOf(Blob);
    expect(got?.type).toBe('image/png');
    await src.delete('any');
  } finally {
    globalThis.fetch = orig;
  }
});
