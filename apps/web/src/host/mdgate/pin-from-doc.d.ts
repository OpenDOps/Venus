import type { Doc } from 'yjs';
import type { Sidecar, fromDoc } from './from-doc.js';

export type YjsPin = {
  bytes: Uint8Array;
  clock: string;
};

export function pinYjsBytes(spaceDoc: Doc): YjsPin;

export function fromPinnedBytes(
  bytes: Uint8Array,
  options?: {
    clock?: string;
    docId?: string;
    blobSources?: { main: { name: string }; shadows?: { name: string }[] };
  },
): Promise<{ markdown: string; sidecar: Sidecar }>;

export function pinThenFromDoc(
  store: Parameters<typeof fromDoc>[0] & {
    blobSync?: { main?: { name: string } };
  },
  workspace: Parameters<typeof fromDoc>[1],
  options?: {
    blobSources?: { main: { name: string }; shadows?: { name: string }[] };
  },
): Promise<{
  pin: YjsPin;
  markdown: string;
  sidecar: Sidecar;
}>;
