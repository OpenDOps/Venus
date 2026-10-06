import { memo, useEffect, useRef, useState } from 'react';
import type { MouseEvent as ReactMouseEvent, MutableRefObject } from 'react';
import {
  dragAndDropFeature,
  hotkeysCoreFeature,
  isOrderedDragTarget,
  renamingFeature,
  selectionFeature,
  syncDataLoaderFeature,
} from '@headless-tree/core';
import { useTree } from '@headless-tree/react';
import type { DragTarget, ItemInstance, TreeInstance } from '@headless-tree/core';
import type { Doc } from 'yjs';
import { PAGE_DOC_ID } from '../ids.js';
import type { SyncProvider } from '../sync-provider.js';
import { testidProps } from '../providers/from-env.js';
import { createPublishedDoc } from './create-published.js';
import {
  applyCatalogDrop,
  canCatalogDrop,
  catalogParentId,
  destFromDrop,
  WIKI_ROOT_ID,
} from './drop.js';
import { createFolder, deleteNode, rename } from './ops.js';
import {
  FOLDER_SPEC_ID,
  KIND_DOC,
  KIND_FOLDER,
  childrenIndex,
  getNode,
  hasChild,
  isHome,
  type CatalogNode,
  type ChildrenIndex,
} from './schema.js';
import { pruneGoneTreeItems } from './tree-prune.js';
import { UNFILED_ID, catalogTreeChildIds } from './repair-structure.js';
import './CatalogTree.css';

const WIKI_ROOT: CatalogNode = {
  id: WIKI_ROOT_ID,
  kind: KIND_FOLDER,
  name: '',
  parentId: null,
  order: '',
  gitName: '',
  gitPath: '',
};

const UNFILED: CatalogNode = {
  id: UNFILED_ID,
  kind: 'unfiled',
  name: 'Unfiled',
  parentId: null,
  order: '',
  gitName: '',
  gitPath: '',
};

function treeItem(catalog: Doc, itemId: string): CatalogNode {
  if (itemId === WIKI_ROOT_ID) return WIKI_ROOT;
  if (itemId === UNFILED_ID) return UNFILED;
  return (
    getNode(catalog, itemId, { gitPath: false }) ?? {
      id: itemId,
      kind: 'unknown',
      name: '',
      parentId: null,
      order: '',
      gitName: '',
      gitPath: '',
    }
  );
}

function dropParentId(target: DragTarget<CatalogNode>): string | null {
  return catalogParentId(target.item.getId());
}

function canRenameRow(item: ItemInstance<CatalogNode>) {
  if (item.getId() === UNFILED_ID) return false;
  const kind = item.getItemData().kind;
  return kind === KIND_DOC || kind === KIND_FOLDER;
}

/** Row click: select + open docs. Folders do not toggle expand (chevron does). */
function activateTreeRow(
  event: ReactMouseEvent,
  item: ItemInstance<CatalogNode>,
  tree: TreeInstance<CatalogNode>,
) {
  const id = item.getId();
  if (event.shiftKey) {
    item.selectUpTo(event.ctrlKey || event.metaKey);
  } else if (event.ctrlKey || event.metaKey) {
    item.toggleSelect();
  } else {
    tree.setSelectedItems([id]);
  }
  if (!event.shiftKey) {
    const dataRef = tree.getDataRef<{ selectUpToAnchorId?: string }>();
    dataRef.current.selectUpToAnchorId = id;
  }
  item.setFocused();
  // Second click of a double-click: select only. Rename starts in startRowRename.
  if (event.detail > 1) return;
  item.primaryAction();
}

function startRowRename(
  event: ReactMouseEvent,
  item: ItemInstance<CatalogNode>,
  tree: TreeInstance<CatalogNode>,
) {
  if (event.shiftKey || event.ctrlKey || event.metaKey) return;
  if (!canRenameRow(item)) return;
  event.preventDefault();
  event.stopPropagation();
  const id = item.getId();
  tree.setSelectedItems([id]);
  item.setFocused();
  item.startRenaming();
}

function PencilIcon() {
  return (
    <svg
      className="venus-tree-edit-icon"
      viewBox="0 0 24 24"
      width="12"
      height="12"
      aria-hidden="true"
      focusable="false"
    >
      <path
        fill="currentColor"
        d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zm17.71-10.21a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
      />
    </svg>
  );
}

function treeItemClassName(
  item: ItemInstance<CatalogNode>,
  selectedDocId: string,
) {
  const id = item.getId();
  let className = 'venus-tree-item';
  if (item.isFolder()) className += ' is-folder';
  if (item.isSelected()) className += ' is-selected';
  if (id === selectedDocId) className += ' is-open';
  if (isHome(id)) className += ' is-home';
  if (item.isDragTarget()) className += ' is-drop';
  return className;
}

type Props = {
  catalog: Doc;
  workspace: unknown;
  provider: SyncProvider;
  selectedDocId: string;
  rebuildRef: MutableRefObject<(() => void) | null>;
  onOpenDoc: (docId: string) => void | Promise<void>;
};

export const CatalogTree = memo(function CatalogTree({
  catalog,
  workspace,
  provider,
  selectedDocId,
  rebuildRef,
  onOpenDoc,
}: Props) {
  const [contextCreateAt, setContextCreateAt] = useState<
    string | null | undefined
  >(undefined);
  const childrenIndexRef = useRef<ChildrenIndex | null>(null);

  function loadChildrenIndex() {
    let index = childrenIndexRef.current;
    if (!index) {
      index = childrenIndex(catalog);
      childrenIndexRef.current = index;
    }
    return index;
  }

  const tree = useTree<CatalogNode>({
    rootItemId: WIKI_ROOT_ID,
    indent: 16,
    initialState: { expandedItems: [FOLDER_SPEC_ID] },
    getItemName: (item) => item.getItemData().name,
    isItemFolder: (item) =>
      item.getId() === WIKI_ROOT_ID ||
      item.getId() === UNFILED_ID ||
      item.getItemData().kind === KIND_FOLDER,
    dataLoader: {
      getItem: (itemId) => treeItem(catalog, itemId),
      getChildren: (itemId) =>
        catalogTreeChildIds(catalog, itemId, loadChildrenIndex()),
    },
    onPrimaryAction: (item) => {
      const data = item.getItemData();
      if (data.kind === 'doc') onOpenDoc(data.docId ?? data.id);
    },
    onRename: (item, value) => {
      if (item.getId() === UNFILED_ID) return;
      rename(catalog, workspace, item.getId(), value);
    },
    canReorder: true,
    canDrop: (items, target) =>
      canCatalogDrop(
        catalog,
        items.map((item) => item.getId()),
        dropParentId(target),
      ),
    onDrop: (items, target) => {
      const draggedIds = items.map((item) => item.getId());
      const dest = destFromDrop(catalog, draggedIds, {
        parentId: dropParentId(target),
        ...(isOrderedDragTarget(target)
          ? { childIndex: target.insertionIndex }
          : {}),
      });
      applyCatalogDrop(catalog, draggedIds, dest);
    },
    features: [
      syncDataLoaderFeature,
      selectionFeature,
      hotkeysCoreFeature,
      dragAndDropFeature,
      renamingFeature,
    ],
  });

  useEffect(() => {
    rebuildRef.current = () => {
      childrenIndexRef.current = childrenIndex(catalog);
      tree.rebuildTree();
      pruneGoneTreeItems(tree, catalog);
      const unfiled = tree.getItems().find((item) => item.getId() === UNFILED_ID);
      if (unfiled && !unfiled.isExpanded()) unfiled.expand();
    };
    return () => {
      rebuildRef.current = null;
    };
  }, [rebuildRef, tree, catalog]);

  function resolveCreateAt(): string | null {
    if (contextCreateAt !== undefined) return contextCreateAt;
    const selected = tree.getSelectedItems()[0] as
      | ItemInstance<CatalogNode>
      | undefined;
    if (!selected) return FOLDER_SPEC_ID;
    const data = selected.getItemData();
    if (data.id === WIKI_ROOT_ID) return null;
    if (data.kind === KIND_FOLDER) return data.id;
    return data.parentId;
  }

  function onCreatePage() {
    const createAt = resolveCreateAt();
    setContextCreateAt(undefined);
    void createPublishedDoc(catalog, workspace, provider, { createAt }).catch(
      (err) => {
        console.error(err);
      },
    );
  }

  function onCreateFolder() {
    createFolder(catalog, { createAt: resolveCreateAt(), name: 'folder' });
    setContextCreateAt(undefined);
  }

  const selected = tree.getSelectedItems()[0] as
    | ItemInstance<CatalogNode>
    | undefined;
  const selectedId = selected?.getId();
  const selectedNode = selectedId
    ? getNode(catalog, selectedId, { gitPath: false })
    : null;
  const showDelete = Boolean(
    selectedNode &&
      !isHome(selectedNode) &&
      !hasChild(catalog, selectedNode.id, loadChildrenIndex()),
  );

  function onDelete() {
    if (!selectedNode || !showDelete) return;
    const id = selectedNode.id;
    try {
      deleteNode(catalog, id);
    } catch {
      return;
    }
    setContextCreateAt(undefined);
    if (id === selectedDocId) onOpenDoc(PAGE_DOC_ID);
  }

  return (
    <div className="venus-tree-shell">
      <div className="venus-tree-toolbar">
        <button type="button" onClick={onCreatePage} {...testidProps('venus-create-page')}>
          Page
        </button>
        <button
          type="button"
          onClick={onCreateFolder}
          {...testidProps('venus-create-folder')}
        >
          Folder
        </button>
        {showDelete ? (
          <button
            type="button"
            onClick={onDelete}
            {...testidProps('venus-delete-node')}
          >
            Delete
          </button>
        ) : null}
      </div>
      <div
        {...tree.getContainerProps('Wiki')}
        {...testidProps('venus-tree')}
        className="venus-tree"
        onContextMenu={(event) => {
          if (event.target !== event.currentTarget) return;
          event.preventDefault();
          setContextCreateAt(null);
        }}
      >
        {tree.getItems().map((item) => {
          const id = item.getId();
          const homeRow = isHome(id);
          const folder = item.isFolder();
          const level = item.getItemMeta().level;
          return (
            <div
              key={item.getKey()}
              className="venus-tree-row"
              data-level={level}
            >
              {folder ? (
                <button
                  type="button"
                  className="venus-tree-chevron"
                  data-catalog-expand={id}
                  aria-label={item.isExpanded() ? 'Collapse' : 'Expand'}
                  aria-expanded={item.isExpanded()}
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    if (item.isExpanded()) item.collapse();
                    else item.expand();
                  }}
                />
              ) : (
                <span className="venus-tree-chevron-spacer" aria-hidden="true" />
              )}
              {item.isRenaming() ? (
                <input
                  {...item.getRenameInputProps()}
                  className="venus-tree-rename"
                  data-catalog-id={id}
                  autoComplete="off"
                  onClick={(event) => event.stopPropagation()}
                  onBlur={() => tree.completeRenaming()}
                  {...(homeRow ? testidProps('venus-tree-home') : {})}
                />
              ) : (
                <button
                  {...item.getProps()}
                  type="button"
                  data-catalog-id={id}
                  className={treeItemClassName(item, selectedDocId)}
                  onClick={(event) => activateTreeRow(event, item, tree)}
                  onDoubleClick={(event) => startRowRename(event, item, tree)}
                  {...(homeRow ? testidProps('venus-tree-home') : {})}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    const data = item.getItemData();
                    setContextCreateAt(
                      id === UNFILED_ID
                        ? null
                        : data.kind === KIND_FOLDER
                          ? data.id
                          : data.parentId,
                    );
                    item.select();
                  }}
                >
                  <span className="venus-tree-label">{item.getItemName()}</span>
                </button>
              )}
              {!item.isRenaming() && canRenameRow(item) ? (
                <button
                  type="button"
                  className="venus-tree-edit"
                  data-catalog-rename={id}
                  aria-label="Rename"
                  onClick={(event) => startRowRename(event, item, tree)}
                >
                  <PencilIcon />
                </button>
              ) : null}
            </div>
          );
        })}
        <div className="venus-tree-drag-line" style={tree.getDragLineStyle()} />
      </div>
    </div>
  );
});
