/**
 * Image upload functionality for gallery attachments.
 */

import type { EditorView } from "@codemirror/view";

import { uploadAttachmentWithProgress } from "../../api/attachments.ts";
import { notifyAttachmentChange } from "./index.ts";
import {
  ENCRYPTED_FORMAT_VERSION,
  encryptBinary,
} from "../../crypto/operations.ts";
import {
  getSessionEncryptionKey,
  isEncryptionEnabled,
} from "../../crypto/keystore.ts";
import {
  convertHeicIfNeeded,
  processImage,
  HeicConversionError,
  type ProcessedImage,
} from "../heic-convert.ts";
import { showError } from "../../toast.ts";
import { GALLERY_PATTERN } from "./patterns.ts";
import { getRequiredElement } from "../../../../shared/dom.ts";
import type { UploadProgress, ProgressCallback } from "./progress.ts";

/**
 * Check if a file is a HEIC/HEIF image.
 */
export function isHeicFile(file: File): boolean {
  return (
    file.type === "image/heic" ||
    file.type === "image/heif" ||
    file.name.toLowerCase().endsWith(".heic") ||
    file.name.toLowerCase().endsWith(".heif")
  );
}

/** Encryption version 0 indicates no encryption */
const UNENCRYPTED_VERSION = 0;

/** Options for uploadProcessedImage */
interface UploadOptions {
  onProgress?: ProgressCallback;
}

/**
 * Upload processed image data (with or without encryption).
 * Returns the UUID of the uploaded attachment.
 * Reports progress through onProgress callback.
 */
async function uploadProcessedImage(
  processed: ProcessedImage,
  options?: UploadOptions,
): Promise<string> {
  const { image, thumbnails } = processed;
  const { onProgress } = options ?? {};

  // Check if encryption is enabled
  if (isEncryptionEnabled()) {
    const sessionEncryptionKey = getSessionEncryptionKey();
    if (!sessionEncryptionKey) {
      throw new Error("Encryption key not available. Please unlock first.");
    }

    // Report encrypting stage
    onProgress?.({ stage: "encrypting" });

    const [encryptedImage, encThumbSm, encThumbMd, encThumbLg] =
      await Promise.all([
        encryptBinary(image, sessionEncryptionKey),
        encryptBinary(thumbnails.sm, sessionEncryptionKey),
        encryptBinary(thumbnails.md, sessionEncryptionKey),
        encryptBinary(thumbnails.lg, sessionEncryptionKey),
      ]);

    const sizes = {
      sm: encThumbSm.ciphertext.byteLength,
      md: encThumbMd.ciphertext.byteLength,
      lg: encThumbLg.ciphertext.byteLength,
      img: encryptedImage.ciphertext.byteLength,
    };

    try {
      // Report uploading stage with 0%
      onProgress?.({ stage: "uploading", percent: 0 });

      const response = await uploadAttachmentWithProgress(
        {
          image: encryptedImage.ciphertext,
          image_iv: encryptedImage.iv,
          thumb_sm: encThumbSm.ciphertext,
          thumb_sm_iv: encThumbSm.iv,
          thumb_md: encThumbMd.ciphertext,
          thumb_md_iv: encThumbMd.iv,
          thumb_lg: encThumbLg.ciphertext,
          thumb_lg_iv: encThumbLg.iv,
          encryption_version: ENCRYPTED_FORMAT_VERSION,
        },
        (percent) => onProgress?.({ stage: "uploading", percent }),
      );

      return response.uuid;
    } catch (err) {
      const error = err as Error & { debugInfo?: string };
      error.debugInfo = `Sizes: sm=${sizes.sm}, md=${sizes.md}, lg=${sizes.lg}, img=${sizes.img}`;
      throw error;
    }
  } else {
    // No encryption - upload raw data
    // Report uploading stage with 0%
    onProgress?.({ stage: "uploading", percent: 0 });

    const response = await uploadAttachmentWithProgress(
      {
        image,
        image_iv: "",
        thumb_sm: thumbnails.sm,
        thumb_sm_iv: "",
        thumb_md: thumbnails.md,
        thumb_md_iv: "",
        thumb_lg: thumbnails.lg,
        thumb_lg_iv: "",
        encryption_version: UNENCRYPTED_VERSION,
      },
      (percent) => onProgress?.({ stage: "uploading", percent }),
    );

    return response.uuid;
  }
}

/** Options for processAndUploadFile */
export interface ProcessAndUploadOptions {
  /** Called with detailed progress updates */
  onProgress?: ProgressCallback;
  /** Called to check if upload was cancelled (e.g., placeholder deleted) */
  isCancelled?: () => boolean;
}

// Re-export types for consumers
export type { UploadProgress, ProgressCallback } from "./progress.ts";

/**
 * Process and upload an image file.
 * Handles HEIC conversion, WebP conversion, thumbnails, and upload.
 * Reports detailed progress through onProgress callback.
 * Returns the UUID of the uploaded attachment, or null if cancelled/failed.
 * Shows user-friendly error messages via toast notifications.
 */
export async function processAndUploadFile(
  file: File,
  options?: ProcessAndUploadOptions,
): Promise<string | null> {
  const { onProgress, isCancelled } = options ?? {};

  try {
    let convertedFile: File;
    try {
      // Report converting stage for HEIC files
      if (isHeicFile(file)) {
        onProgress?.({ stage: "converting" });
      }
      // Convert HEIC to WebP first (if needed)
      convertedFile = await convertHeicIfNeeded(file);
      // Check if cancelled during conversion
      if (isCancelled?.()) {
        return null;
      }
    } catch (err) {
      console.error("Failed to convert HEIC:", err);
      if (err instanceof HeicConversionError) {
        showError(err.message);
      } else {
        showError("Failed to process image. Please try a different format.");
      }
      return null;
    }

    // Report compressing stage (processImage does both compression and thumbnails)
    onProgress?.({ stage: "compressing" });

    // Process image: convert to WebP, compress, generate thumbnails (all in parallel)
    let processed: ProcessedImage;
    try {
      processed = await processImage(convertedFile);
    } catch (err) {
      console.error("Failed to process image:", err);
      showError("Failed to process image. Please try a different format.");
      return null;
    }

    // Check if cancelled before upload
    if (isCancelled?.()) {
      return null;
    }

    try {
      // uploadProcessedImage handles encrypting and uploading stages
      const uuid = await uploadProcessedImage(processed, { onProgress });
      return uuid;
    } catch (err) {
      console.error("Failed to upload image:", err);
      const message =
        err instanceof Error ? err.message : "Unknown error occurred";
      const debugInfo = (err as { debugInfo?: string }).debugInfo ?? "";
      showError(
        `Failed to upload image: ${message}${debugInfo ? ` | ${debugInfo}` : ""}`,
      );
      return null;
    }
  } catch (err) {
    console.error("Unexpected error during image upload:", err);
    showError("An unexpected error occurred. Please try again.");
    return null;
  }
}

/** Get the hidden file input element from the DOM */
function getFileInput(): HTMLInputElement {
  return getRequiredElement("image-upload-input", HTMLInputElement);
}

/** Current upload handler - replaced each time we trigger an upload */
let currentUploadHandler: ((e: Event) => void) | null = null;

/**
 * Trigger the file input with a custom handler.
 * Manages event listener cleanup automatically.
 */
export function triggerFileInput(handler: (files: File[]) => void): void {
  const input = getFileInput();

  // Remove previous handler if any
  if (currentUploadHandler) {
    input.removeEventListener("change", currentUploadHandler);
  }

  // Reset the input value so the same file can be selected again
  input.value = "";

  const wrappedHandler = () => {
    const files = input.files;
    if (files && files.length > 0) {
      handler(Array.from(files));
    }
  };

  currentUploadHandler = wrappedHandler;
  input.addEventListener("change", wrappedHandler);
  input.click();
}

import { getProgressText, type UploadStage } from "./progress.ts";

/** Unique ID counter for upload placeholders */
let uploadIdCounter = 0;

/**
 * Generate a unique upload ID for tracking placeholders.
 */
function generateUploadId(): string {
  return `upload-${++uploadIdCounter}`;
}

/**
 * Create placeholder text for a given upload ID and stage.
 * Format: ![stage:percent](attachment:upload-id)
 */
function createPlaceholderText(
  uploadId: string,
  stage: UploadStage,
  percent?: number,
): string {
  const stageText =
    stage === "uploading" && percent !== undefined
      ? `${stage}:${percent}`
      : stage;
  return `![${stageText}](attachment:${uploadId})`;
}

/**
 * Find a placeholder by upload ID in the document.
 */
function findPlaceholder(
  view: EditorView,
  uploadId: string,
): { from: number; to: number; text: string } | null {
  const doc = view.state.doc.toString();
  // Match any placeholder with this upload ID
  const pattern = new RegExp(`!\\[[^\\]]*\\]\\(attachment:${uploadId}\\)`, "g");
  const match = pattern.exec(doc);
  if (match) {
    return {
      from: match.index,
      to: match.index + match[0].length,
      text: match[0],
    };
  }
  return null;
}

/**
 * Update a placeholder with new progress state.
 */
function updatePlaceholder(
  view: EditorView,
  uploadId: string,
  stage: UploadStage,
  percent?: number,
): boolean {
  const placeholder = findPlaceholder(view, uploadId);
  if (!placeholder) return false;

  const newText = createPlaceholderText(uploadId, stage, percent);
  if (newText !== placeholder.text) {
    view.dispatch({
      changes: { from: placeholder.from, to: placeholder.to, insert: newText },
    });
  }
  return true;
}

/**
 * Process a single file and insert/update the placeholder in the editor.
 * If galleryCreated is false, creates a new gallery. Otherwise appends to existing.
 * Returns the UUID if successful, null if failed/cancelled.
 */
async function uploadSingleFile(
  view: EditorView,
  file: File,
  insertPos: number,
  galleryCreated: boolean,
): Promise<string | null> {
  // Generate unique ID for this upload
  const uploadId = generateUploadId();

  // Initial stage: converting for HEIC, compressing for others
  const initialStage: UploadStage = isHeicFile(file)
    ? "converting"
    : "compressing";
  const initialPlaceholder = createPlaceholderText(uploadId, initialStage);

  if (galleryCreated) {
    // Append to existing gallery - insert before the closing ::
    const doc = view.state.doc.toString();
    GALLERY_PATTERN.lastIndex = 0;
    let match: RegExpExecArray | null;
    let lastMatchEnd = -1;

    while ((match = GALLERY_PATTERN.exec(doc)) !== null) {
      if (match.index >= insertPos - 50) {
        // Found a gallery near our insert position
        lastMatchEnd = match.index + match[0].length;
        break;
      }
    }

    if (lastMatchEnd !== -1) {
      // Insert before the closing ::
      const appendPos = lastMatchEnd - 2;
      view.dispatch({
        changes: { from: appendPos, to: appendPos, insert: initialPlaceholder },
      });
    }
  } else {
    // Create new gallery
    const loadingGallery = `\n::gallery{}${initialPlaceholder}::`;
    view.dispatch({
      changes: { from: insertPos, to: insertPos, insert: loadingGallery },
    });
  }

  const uuid = await processAndUploadFile(file, {
    onProgress: (progress) => {
      updatePlaceholder(view, uploadId, progress.stage, progress.percent);
    },
    isCancelled: () => findPlaceholder(view, uploadId) === null,
  });

  // uuid is null on cancel or error - clean up placeholder if it still exists
  if (uuid === null) {
    const placeholder = findPlaceholder(view, uploadId);
    if (placeholder) {
      view.dispatch({
        changes: { from: placeholder.from, to: placeholder.to, insert: "" },
      });

      // Clean up empty galleries
      const emptyGallery = "\n::gallery{}::";
      const emptyIndex = view.state.doc.toString().indexOf(emptyGallery);
      if (emptyIndex !== -1) {
        view.dispatch({
          changes: {
            from: emptyIndex,
            to: emptyIndex + emptyGallery.length,
            insert: "",
          },
        });
      }
    }
    return null;
  }

  // Replace the placeholder with the final image
  const placeholder = findPlaceholder(view, uploadId);
  if (placeholder) {
    const newImage = `![image](attachment:${uuid})`;
    view.dispatch({
      changes: { from: placeholder.from, to: placeholder.to, insert: newImage },
    });
    notifyAttachmentChange();
  }

  return uuid;
}

// Export getProgressText for widget to use
export { getProgressText };

/**
 * Trigger an image upload via file picker.
 * Opens a file dialog, uploads the selected images, and inserts them into a single gallery.
 * Multiple files can be selected but they are processed sequentially.
 * If one fails, continues to the next.
 */
export function triggerImageUpload(view: EditorView): void {
  // Get the end of the current line to insert after it
  const cursorPos = view.state.selection.main.head;
  const currentLine = view.state.doc.lineAt(cursorPos);
  const insertPos = currentLine.to;

  triggerFileInput(async (files) => {
    let galleryCreated = false;

    // Process files one at a time, all into the same gallery
    for (const file of files) {
      const uuid = await uploadSingleFile(
        view,
        file,
        insertPos,
        galleryCreated,
      );
      if (uuid !== null) {
        galleryCreated = true;
      }
    }
  });
}
