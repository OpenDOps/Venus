/** Stored parent replaced by root when the node is an orphan or the cycle break. */
export function repairedParent(
  id: string,
  links: Map<string, string | null>,
): string | null;
