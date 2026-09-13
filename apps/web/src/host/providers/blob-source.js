import { WORKSPACE_ID } from '../ids.js';

/**
 * BlobSource → hub POST/GET /api/blobs/:workspace. Do not import this from
 * mount-editor.js. No list route. Hash is SHA-256 base64url with
 * padding (same as BlockSuite `sha()`; confirmed against M1 keck 276e0e9).
 */

export function blobOriginFromSyncUrl(syncUrl, { sameOrigin = false } = {}) {
  if (sameOrigin) return '';
  const u = new URL(syncUrl);
  u.protocol = u.protocol === 'wss:' ? 'https:' : 'http:';
  return `${u.protocol}//${u.host}`;
}

function sniffType(buf) {
  const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
  if (u8.length >= 8 && u8[0] === 0x89 && u8[1] === 0x50 && u8[2] === 0x4e) {
    return 'image/png';
  }
  if (u8.length >= 3 && u8[0] === 0xff && u8[1] === 0xd8 && u8[2] === 0xff) {
    return 'image/jpeg';
  }
  if (u8.length >= 6 && u8[0] === 0x47 && u8[1] === 0x49 && u8[2] === 0x46) {
    return 'image/gif';
  }
  if (
    u8.length >= 12 &&
    u8[0] === 0x52 &&
    u8[8] === 0x57 &&
    u8[9] === 0x45 &&
    u8[10] === 0x42 &&
    u8[11] === 0x50
  ) {
    return 'image/webp';
  }
  return 'application/octet-stream';
}

export class OctoBaseBlobSource {
  name = 'octobase';
  readonly = false;

  /**
   * @param {{ workspaceId?: string, origin?: string }} [options]
   *   `origin` empty → same-origin `/api/blobs/...` (Vite proxies to keck).
   */
  constructor({ workspaceId = WORKSPACE_ID, origin = '' } = {}) {
    this.workspaceId = workspaceId;
    this.origin = origin.replace(/\/$/, '');
  }

  _url(hash) {
    const path = hash
      ? `/api/blobs/${this.workspaceId}/${hash}`
      : `/api/blobs/${this.workspaceId}`;
    return `${this.origin}${path}`;
  }

  async get(key) {
    const res = await fetch(this._url(key));
    if (res.status === 404) return null;
    if (!res.ok) {
      throw new Error(`blob GET ${res.status} ${this._url(key)}`);
    }
    const buf = await res.arrayBuffer();
    const headerType = res.headers.get('content-type') || '';
    const type =
      headerType.startsWith('image/') ? headerType : sniffType(buf);
    return new Blob([buf], { type });
  }

  async set(key, value) {
    const body = await value.arrayBuffer();
    const res = await fetch(this._url(), {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body,
    });
    if (!res.ok) {
      throw new Error(
        `blob POST ${res.status} ${this._url()}. Is Compose hub up?`,
      );
    }
    const json = await res.json().catch(() => null);
    const id = json && typeof json.id === 'string' ? json.id : '';
    if (id && id !== key) {
      console.warn(
        `keck blob id ${id} !== BlockSuite key ${key}; GET uses the BlockSuite key`,
      );
    }
    return key;
  }

  async delete(key) {
    const res = await fetch(this._url(key), { method: 'DELETE' });
    if (res.status === 404 || res.status === 204 || res.ok) return;
    throw new Error(`blob DELETE ${res.status} ${this._url(key)}`);
  }

  list() {
    return Promise.resolve([]);
  }
}
