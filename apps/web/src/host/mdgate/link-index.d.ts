export type LinkIndex = {
  inbound: Record<string, string[]>;
  outbound: Record<string, string[]>;
};

export function emptyIndex(): LinkIndex;
export function targetsInMarkdown(markdown: string): string[];
export function inbound(index: LinkIndex, targetDocId: string): string[];
export function outbound(index: LinkIndex, sourceDocId: string): string[];
export function upsertOutbound(
  index: LinkIndex,
  sourceId: string,
  targets: string[],
): LinkIndex;
export function dropId(index: LinkIndex, docId: string): LinkIndex;
export function buildFromPages(
  pages: Array<{ id: string; markdown: string }>,
): LinkIndex;
export function serialize(index: LinkIndex): string;
export function parse(json: string): LinkIndex | null;
export function pathToDocIdFromPagesYaml(yaml: string): Record<string, string>;
export function rebuildFromWiki(wikiDir: string): LinkIndex;
export function linksJsonRel(): string;
