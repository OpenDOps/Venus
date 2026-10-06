import type { Doc } from 'yjs';
import type { CatalogNode, ChildrenIndex } from './schema.js';

export const UNFILED_ID: 'wiki:unfiled';

export function parentLinks(catalog: Doc): Map<string, string | null>;

export function orphanNodes(catalog: Doc, index?: ChildrenIndex): CatalogNode[];

export function catalogTreeChildIds(
  catalog: Doc,
  itemId: string,
  index: ChildrenIndex,
): string[];

/** Rewrite orphan and cycle-break parents to the root. Returns whether it wrote. */
export function repairCatalogStructure(catalog: Doc): boolean;
