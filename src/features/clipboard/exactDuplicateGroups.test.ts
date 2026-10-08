import { describe, expect, it } from "vitest";
import {
  exactDuplicateGroupsToExpand,
  flattenExactDuplicateGroups,
  type GroupedClipboardItem,
  groupExactDuplicates,
} from "./exactDuplicateGroups";

function clip(id: string, createdAt: string, groupId?: string | null): GroupedClipboardItem {
  return {
    id,
    content: `clip ${id}`,
    kind: "text",
    sourceApp: "Notes",
    createdAt,
    updatedAt: createdAt,
    isPinned: false,
    copyCount: 0,
    groupId,
  };
}

describe("exact duplicate groups", () => {
  it("uses only storage-provided IDs and shows the newest version first", () => {
    const groups = groupExactDuplicates([
      clip("older", "2026-07-20T09:00:00.000Z", "exact-a"),
      clip("other", "2026-07-21T09:00:00.000Z", "exact-b"),
      clip("newer", "2026-07-22T09:00:00.000Z", "exact-a"),
    ]);

    expect(groups).toHaveLength(2);
    expect(groups[0]).toMatchObject({ groupId: "exact-a", versionCount: 2 });
    expect(groups[0]?.representative.id).toBe("newer");
    expect(groups[0]?.versions.map((item) => item.id)).toEqual(["newer", "older"]);
    expect(groups[0]?.isCollapsedByDefault).toBe(true);
  });

  it("does not infer a group from visible content", () => {
    const first = { ...clip("first", "2026-07-22T09:00:00.000Z"), content: "same text" };
    const second = { ...clip("second", "2026-07-21T09:00:00.000Z"), content: "same text" };

    const groups = groupExactDuplicates([first, second]);

    expect(groups).toHaveLength(2);
    expect(groups.every((group) => group.versionCount === 1)).toBe(true);
  });

  it("expands the matching group and keeps other groups collapsed", () => {
    const groups = groupExactDuplicates([
      clip("first", "2026-07-20T09:00:00.000Z", "exact-a"),
      clip("matched-old", "2026-07-19T09:00:00.000Z", "exact-a"),
      clip("latest-b", "2026-07-22T09:00:00.000Z", "exact-b"),
      clip("old-b", "2026-07-18T09:00:00.000Z", "exact-b"),
    ]);

    const expanded = exactDuplicateGroupsToExpand(groups, ["matched-old"]);
    expect(expanded).toEqual(new Set(["exact-a"]));
    expect(flattenExactDuplicateGroups(groups, expanded).map((item) => item.id)).toEqual([
      "latest-b",
      "first",
      "matched-old",
    ]);
  });
});
