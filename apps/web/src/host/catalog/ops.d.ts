import type { Doc } from 'yjs';
import type { CatalogNode } from './schema.js';

export class CatalogError extends Error {
  readonly code: string;
  constructor(code: string, message?: string);
}

export function seedOnce(catalog: Doc, workspace?: unknown): void;

export function mintDocId(
  catalog: Doc,
  createAt: string | null | undefined,
): string;

export function commitNewDoc(
  catalog: Doc,
  workspace: unknown,
  stage: { id: string; createAt: string | null },
): CatalogNode;

export function createDoc(
  catalog: Doc,
  workspace: unknown,
  options: { createAt: string | null },
): CatalogNode;

export function createFolder(
  catalog: Doc,
  options: { createAt: string | null; name: string },
): CatalogNode;

export function rename(
  catalog: Doc,
  workspace: unknown,
  id: string,
  name: string,
): CatalogNode;

export function reparent(
  catalog: Doc,
  id: string,
  dest: {
    parentId: string | null;
    afterId?: string | null;
    beforeId?: string | null;
  },
): CatalogNode;

export function setOrder(
  catalog: Doc,
  id: string,
  pos?: { afterId?: string | null; beforeId?: string | null },
): CatalogNode;

export function deleteNode(catalog: Doc, id: string): void;
