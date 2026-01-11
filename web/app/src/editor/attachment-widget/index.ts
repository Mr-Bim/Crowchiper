/**
 * Gallery widget decorations for CodeMirror.
 *
 * Handles image galleries with the format:
 * ::gallery{}![alt](attachment:uuid1)![alt](attachment:uuid2)::
 *
 * - Multiple images displayed in a row with individual delete buttons
 * - Cursor can be positioned between images
 * - Backspace deletes the image before cursor
 * - When last image is deleted, the whole gallery is removed
 * - Empty JSON {} reserved for future styling options
 */

export { clearImageCache, clearImageCacheExcept } from "./cache.ts";
export { triggerImageUpload } from "./upload.ts";

import { galleryViewPlugin, atomicRangesPlugin } from "./decorations.ts";
import {
  galleryLines,
  galleryKeyHandler,
  galleryCursorGuard,
  galleryInputHandler,
} from "./keyboard.ts";

/**
 * Gallery plugin that provides:
 * - Widget decorations for gallery images
 * - Atomic range behavior
 * - Custom backspace handling
 * - Input redirection (typing on gallery lines goes to next line)
 * - Cached gallery line numbers (via state field)
 */
export const attachmentPlugin = [
  galleryLines,
  galleryViewPlugin,
  atomicRangesPlugin,
  galleryKeyHandler,
  galleryCursorGuard,
  galleryInputHandler,
];

/**
 * Parse attachment UUIDs from content.
 * Used when saving posts to update reference counts.
 */
export function parseAttachmentUuids(content: string): string[] {
  const uuids: string[] = [];
  let match: RegExpExecArray | null;
  const pattern = /!\[[^\]]*\]\(attachment:([a-f0-9-]+)\)/g;

  while ((match = pattern.exec(content)) !== null) {
    if (match[1] !== "pending") {
      uuids.push(match[1]);
    }
  }

  return [...new Set(uuids)];
}

/**
 * Callback triggered when attachments are uploaded or deleted.
 * Used to trigger an immediate save to server.
 */
let onAttachmentChangeCallback: (() => void) | null = null;

export function setOnAttachmentChange(callback: () => void): void {
  onAttachmentChangeCallback = callback;
}

export function notifyAttachmentChange(): void {
  onAttachmentChangeCallback?.();
}
