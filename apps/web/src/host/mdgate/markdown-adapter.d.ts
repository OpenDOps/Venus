export type MarkdownAdapterHost = {
  fromDoc: (
    store: unknown,
  ) => Promise<{ file: string; assetsIds: string[] } | undefined>;
  fromDocSnapshot: (payload: {
    snapshot: unknown;
    assets?: unknown;
  }) => Promise<{ file: string; assetsIds?: string[] } | undefined>;
  fromBlockSnapshot: (payload: {
    snapshot: unknown;
    assets?: unknown;
  }) => Promise<{ file?: string } | undefined>;
  toDoc: (payload: { file: string }) => Promise<unknown>;
  job: {
    docToSnapshot: (store: unknown) => { blocks: unknown } | undefined;
    blockToSnapshot: (model: unknown) => unknown;
    assetsManager: unknown;
  };
};

export function createMarkdownAdapter(
  store: {
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
  },
  workspace: { id: string; meta: { docMetas: unknown[] } },
): MarkdownAdapterHost;

/** Cached `createMarkdownAdapter` keyed by Store + workspace id / docMetas. */
export function markdownAdapterFor(
  store: {
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
  },
  workspace: { id: string; meta: { docMetas: unknown[] } },
): MarkdownAdapterHost;

type NoteStore = {
  addBlock: (
    flavour: string,
    props: Record<string, unknown>,
    parentId?: string,
    parentIndex?: number,
  ) => string;
  deleteBlock: (model: string | { id: string }) => void;
  updateBlock: (id: string, props: Record<string, unknown>) => void;
  getBlock?: (id: string) => {
    model?: {
      text?: {
        toString?: () => string;
        toDelta?: () => unknown;
        delete?: (index: number, length: number) => void;
        insert?: (text: string, index: number) => void;
        format?: (
          index: number,
          length: number,
          attrs: Record<string, unknown>,
        ) => void;
      };
    };
  };
};

type Note = { id: string; children: { id: string }[] };

export function addListItem(
  store: NoteStore,
  parentId: string,
  type: string,
  text: string,
  extra?: Record<string, unknown>,
): string;

export function addNestedBulletedList(
  store: NoteStore,
  noteId: string,
  outer: string,
  inner: string,
): { outerId: string; innerId: string };

export function clearNote(store: NoteStore, note: Note): void;

export function replaceNoteWithParagraphs(
  store: NoteStore,
  note: Note,
  texts: string[],
): string[];

export function addHeading(
  store: NoteStore,
  noteId: string,
  type: string,
  text: string,
): string;

export function addParagraph(
  store: NoteStore,
  noteId: string,
  text: string,
): string;

export function addParagraphAt(
  store: NoteStore,
  noteId: string,
  text: string,
  index: number,
): string;

export function addImageBlock(
  store: NoteStore,
  noteId: string,
  sourceId: string,
): string;

export function addColoredParagraph(
  store: NoteStore,
  noteId: string,
  text: string,
  color: string,
): string;

export function paragraphHasColorMark(store: NoteStore, blockId: string): boolean;

export function setParagraphText(
  store: NoteStore,
  blockId: string,
  text: string,
): void;

export function setParagraphType(
  store: NoteStore,
  blockId: string,
  type: string,
): void;

export function formatParagraph(
  store: NoteStore,
  blockId: string,
  attrs: Record<string, unknown>,
): void;

export function paragraphTextLength(store: NoteStore, blockId: string): number;

export function addCodeBlock(
  store: NoteStore,
  noteId: string,
  language: string,
  source: string,
): string;

export function addLinkParagraph(
  store: NoteStore,
  noteId: string,
  label: string,
  url: string,
): string;

export function addMarksParagraph(store: NoteStore, noteId: string): string;

export function addEmbedLinkedDoc(
  store: NoteStore,
  noteId: string,
  pageId: string,
): string;
