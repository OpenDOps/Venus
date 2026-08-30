export function blobOriginFromSyncUrl(
  syncUrl: string,
  options?: { sameOrigin?: boolean },
): string;

export class OctoBaseBlobSource {
  readonly name: 'octobase';
  readonly readonly: false;
  readonly origin: string;
  constructor(options?: { workspaceId?: string; origin?: string });
  get(key: string): Promise<Blob | null>;
  set(key: string, value: Blob): Promise<string>;
  delete(key: string): Promise<void>;
  list(): Promise<string[]>;
}
