/**
 * Attachment utility functions.
 *
 * These utilities are used by both the main bundle (posts/save.ts, posts/selection.ts)
 * and the lazy-loaded editor chunk (editor/attachment-widget/).
 *
 * Kept separate to avoid pulling editor dependencies into the main bundle.
 */

/**
 * Parse attachment UUIDs from content.
 * Used when saving posts to update reference counts.
 */
export function parseAttachmentUuids(content: string): string[] {
  const uuids: string[] = [];
  let match: RegExpExecArray | null;
  const pattern = /!\[[^\]]*\]\(attachment:([a-f0-9-]+)\)/g;

  while ((match = pattern.exec(content)) !== null) {
    if (match[1] !== "pending" && match[1] !== "converting") {
      uuids.push(match[1]);
    }
  }

  return [...new Set(uuids)];
}

/**
 * Remove pending/converting image placeholders from content.
 */
export function cleanupPendingUploads(content: string): string {
  // Remove pending/converting image placeholders
  let cleaned = content.replace(
    /!\[(uploading\.\.\.|converting\.\.\.)\]\(attachment:(pending|converting)\)/g,
    "",
  );

  // Remove empty galleries (galleries with no images left)
  cleaned = cleaned.replace(/::gallery\{\}::/g, "");

  return cleaned;
}
