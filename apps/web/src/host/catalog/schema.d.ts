import type { Doc, Map as YMap } from 'yjs';

export const NODES_KEY: 'nodes';
export const FOLDER_SPEC_ID: 'folder:spec';
export const KIND_FOLDER: 'folder';
export const KIND_DOC: 'doc';

export type CatalogKind = 'folder' | 'doc';

export type CatalogNode = {
  id: string;
  kind: string;
  name: string;
  parentId: string | null;
  order: string;
  gitName: string;
  gitPath: string;
  docId?: string;
};

export function nodesMap(catalog: Doc): YMap<YMap<unknown>>;
export function storedGitName(ymap: YMap<unknown>): string;
export function readNode(ymap: YMap<unknown> | null | undefined): CatalogNode | null;
export function gitPathOf(
  catalog: Doc,
  id: string,
  memo?: Map<string, string>,
  visiting?: Set<string>,
): string;
export function getNode(
  catalog: Doc,
  id: string,
  opts?: { gitPath?: boolean },
): CatalogNode | null;
export function listNodes(catalog: Doc): CatalogNode[];
export function compareNodes(
  a: { order: string; id: string },
  b: { order: string; id: string },
): number;
/** parentId → children sorted by order, then id. Built in one scan; no gitPath join. */
export type ChildrenIndex = Map<string | null, CatalogNode[]>;
export function childrenIndex(catalog: Doc): ChildrenIndex;
export function childrenOf(
  catalog: Doc,
  parentId: string | null,
  index?: ChildrenIndex,
): CatalogNode[];
export function hasChild(
  catalog: Doc,
  parentId: string | null,
  index?: ChildrenIndex,
): boolean;
export function isHome(
  nodeOrId: string | { id?: string; docId?: string } | null | undefined,
): boolean;
export function writeNodeFields(
  ymap: YMap<unknown>,
  fields: Record<string, unknown>,
): void;
export function putNode(
  catalog: Doc,
  fields: Record<string, unknown> & { id: string },
): void;
