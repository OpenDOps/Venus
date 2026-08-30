export const SEED_TITLE: string;
export const SEED_H1: string;
export const SEED_H1_BODY: string;
export const SEED_H2: string;
export const SEED_H2_BODY: string;
export const SEED_SPACER_COUNT: number;
export const SEED_MD_DEMO_H2: string;

export function seedHomeNote(store: unknown, noteId: string): void;
export function noteHasMarkdownDemo(store: unknown): boolean;
export function seedMarkdownDemo(store: unknown): Promise<boolean>;
