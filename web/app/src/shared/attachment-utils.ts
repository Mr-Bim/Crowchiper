/**
 * Attachment utility functions.
 *
 * These utilities are used by both the main bundle (posts/save.ts, posts/selection.ts)
 * and the lazy-loaded editor chunk (editor/attachment-widget/).
 *
 * Kept separate to avoid pulling editor dependencies into the main bundle.
 */

/**
 * Active upload tracking for abort functionality.
 * Maps upload ID to AbortController.
 * This is in shared so both main bundle and editor chunk can access it.
 */
const activeUploads = new Map<string, AbortController>();

/**
 * Register an upload with its abort controller.
 * Returns the AbortController for use in the upload.
 */
export function registerUpload(uploadId: string): AbortController {
  const controller = new AbortController();
  activeUploads.set(uploadId, controller);
  return controller;
}

/**
 * Unregister an upload (called when upload completes or fails).
 */
export function unregisterUpload(uploadId: string): void {
  activeUploads.delete(uploadId);
}

/**
 * Abort all active uploads.
 * Called when switching posts or cleaning up.
 */
export function abortAllUploads(): void {
  for (const [id, controller] of activeUploads) {
    controller.abort();
    activeUploads.delete(id);
  }
}

/**
 * Abort a specific upload by ID.
 */
export function abortUpload(uploadId: string): void {
  const controller = activeUploads.get(uploadId);
  if (controller) {
    controller.abort();
    activeUploads.delete(uploadId);
  }
}

/**
 * Check if there are any active uploads.
 */
export function hasActiveUploads(): boolean {
  return activeUploads.size > 0;
}

/**
 * Check if a UUID is an upload placeholder (not a real attachment).
 * Placeholders use formats like: upload-N, widget-upload-N, pending, converting
 */
export function isUploadPlaceholder(uuid: string): boolean {
  return (
    uuid === "pending" ||
    uuid === "converting" ||
    uuid.startsWith("upload-") ||
    uuid.startsWith("widget-upload-")
  );
}

/**
 * Parse attachment UUIDs from content.
 * Used when saving posts to update reference counts.
 * Excludes upload placeholders (pending, converting, upload-N, widget-upload-N).
 */
export function parseAttachmentUuids(content: string): string[] {
  const uuids: string[] = [];
  let match: RegExpExecArray | null;
  const pattern = /!\[[^\]]*\]\(attachment:([^)]+)\)/g;

  while ((match = pattern.exec(content)) !== null) {
    const uuid = match[1];
    if (!isUploadPlaceholder(uuid)) {
      uuids.push(uuid);
    }
  }

  return [...new Set(uuids)];
}

/**
 * Remove upload placeholder images from content.
 * Handles both old format (pending/converting) and new format (upload-N/widget-upload-N).
 */
export function cleanupPendingUploads(content: string): string {
  // Remove all upload placeholder images:
  // - Old format: ![uploading...](attachment:pending), ![converting...](attachment:converting)
  // - New format: ![stage](attachment:upload-N), ![uploading:50](attachment:widget-upload-N)
  let cleaned = content.replace(
    /!\[[^\]]*\]\(attachment:(pending|converting|upload-\d+|widget-upload-\d+)\)/g,
    "",
  );

  // Remove empty galleries (galleries with no images left)
  cleaned = cleaned.replace(/::gallery\{\}::/g, "");

  return cleaned;
}
