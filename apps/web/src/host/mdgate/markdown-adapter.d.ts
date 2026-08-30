export function createMarkdownAdapter(
  store: {
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
  },
  workspace: { id: string; meta: { docMetas: unknown[] } },
): {
  fromDoc: (
    store: unknown,
  ) => Promise<{ file: string; assetsIds: string[] } | undefined>;
  toDoc: (payload: { file: string }) => Promise<unknown>;
};

type NoteStore = {
  addBlock: (
    flavour: string,
    props: Record<string, unknown>,
    parentId?: string,
  ) => string;
  deleteBlock: (model: string | { id: string }) => void;
};

type Note = { id: string; children: { id: string }[] };

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
