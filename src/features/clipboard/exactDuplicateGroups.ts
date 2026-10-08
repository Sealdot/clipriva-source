import type { ClipboardItem } from "./types";

/**
 * The repository, not the webview, decides whether two records are exact
 * duplicates. Keeping the opaque group ID at this boundary prevents a UI
 * convenience feature from silently normalizing whitespace, changing case, or
 * deriving another content fingerprint.
 */
export interface GroupedClipboardItem extends ClipboardItem {
  groupId?: string | null;
  version?: number;
}

export interface ExactDuplicateGroup {
  /** Null means this record is deliberately not groupable. */
  groupId: string | null;
  representative: GroupedClipboardItem;
  versions: GroupedClipboardItem[];
  versionCount: number;
  /** A group with one version remains a normal history row. */
  isCollapsedByDefault: boolean;
}

/**
 * Builds visual groups from the storage-provided opaque `groupId` only.
 * Items without a group ID stay separate, even when their visible content is
 * identical. That makes the precise-grouping boundary testable and avoids any
 * browser-side guesswork about clipboard data.
 */
export function groupExactDuplicates(items: GroupedClipboardItem[]): ExactDuplicateGroup[] {
  const groups = new Map<string, GroupedClipboardItem[]>();

  for (const item of items) {
    const key = item.groupId ? `group:${item.groupId}` : `item:${item.id}`;
    const versions = groups.get(key) ?? [];
    versions.push(item);
    groups.set(key, versions);
  }

  return [...groups.values()]
    .map((versions) => {
      const sortedVersions = [...versions].sort(compareClipboardVersions);
      const representative = sortedVersions[0];
      if (!representative) {
        throw new Error("Exact duplicate groups cannot be empty.");
      }
      return {
        groupId: representative.groupId ?? null,
        representative,
        versions: sortedVersions,
        versionCount: sortedVersions.length,
        isCollapsedByDefault: sortedVersions.length > 1,
      };
    })
    .sort((left, right) => compareClipboardVersions(left.representative, right.representative));
}

/**
 * A non-empty search must expose the matching historical version. Consumers
 * pass the IDs returned by the repository's search rather than matching raw
 * content again in the UI.
 */
export function exactDuplicateGroupsToExpand(
  groups: ExactDuplicateGroup[],
  matchingItemIds: Iterable<string>,
): Set<string> {
  const matchingIds = new Set(matchingItemIds);
  return new Set(
    groups.flatMap((group) =>
      group.groupId &&
      group.versionCount > 1 &&
      group.versions.some((item) => matchingIds.has(item.id))
        ? [group.groupId]
        : [],
    ),
  );
}

export function flattenExactDuplicateGroups(
  groups: ExactDuplicateGroup[],
  expandedGroupIds: ReadonlySet<string>,
): GroupedClipboardItem[] {
  return groups.flatMap((group) => {
    if (group.groupId && group.isCollapsedByDefault && !expandedGroupIds.has(group.groupId)) {
      return [group.representative];
    }
    return group.versions;
  });
}

function compareClipboardVersions(left: GroupedClipboardItem, right: GroupedClipboardItem) {
  return (
    right.createdAt.localeCompare(left.createdAt) ||
    right.updatedAt.localeCompare(left.updatedAt) ||
    right.id.localeCompare(left.id)
  );
}
