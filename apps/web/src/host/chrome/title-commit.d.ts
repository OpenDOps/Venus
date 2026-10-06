export const TITLE_COMMIT_MS: number;

export function createTitleCommit(
  write: (text: string) => unknown,
  clock?: {
    later?: (fn: () => void, ms: number) => unknown;
    cancel?: (id: unknown) => void;
    delay?: number;
  },
): {
  push: (text: string) => void;
  flush: () => unknown;
  cancel: () => void;
};
