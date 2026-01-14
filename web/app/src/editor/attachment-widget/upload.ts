/**
 * Image upload functionality for gallery attachments.
 */

import type { EditorView } from "@codemirror/view";

import { uploadAttachment } from "../../api/attachments.ts";
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

/**
 * Upload processed image data (with or without encryption).
 * Returns the UUID of the uploaded attachment.
 */
async function uploadProcessedImage(
  processed: ProcessedImage,
): Promise<string> {
  const { image, thumbnails } = processed;

  // Check if encryption is enabled
  if (isEncryptionEnabled()) {
    const sessionEncryptionKey = getSessionEncryptionKey();
    if (!sessionEncryptionKey) {
      throw new Error("Encryption key not available. Please unlock first.");
    }

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
      const response = await uploadAttachment({
        image: encryptedImage.ciphertext,
        image_iv: encryptedImage.iv,
        thumb_sm: encThumbSm.ciphertext,
        thumb_sm_iv: encThumbSm.iv,
        thumb_md: encThumbMd.ciphertext,
        thumb_md_iv: encThumbMd.iv,
        thumb_lg: encThumbLg.ciphertext,
        thumb_lg_iv: encThumbLg.iv,
        encryption_version: ENCRYPTED_FORMAT_VERSION,
      });

      return response.uuid;
    } catch (err) {
      const error = err as Error & { debugInfo?: string };
      error.debugInfo = `Sizes: sm=${sizes.sm}, md=${sizes.md}, lg=${sizes.lg}, img=${sizes.img}`;
      throw error;
    }
  } else {
    // No encryption - upload raw data
    const response = await uploadAttachment({
      image,
      image_iv: "",
      thumb_sm: thumbnails.sm,
      thumb_sm_iv: "",
      thumb_md: thumbnails.md,
      thumb_md_iv: "",
      thumb_lg: thumbnails.lg,
      thumb_lg_iv: "",
      encryption_version: UNENCRYPTED_VERSION,
    });

    return response.uuid;
  }
}

/** Processing state for callbacks */
export type ProcessingState = "converting" | "pending";

/** Options for processAndUploadFile */
export interface ProcessAndUploadOptions {
  /** Called when state changes (e.g., converting -> pending) */
  onStateChange?: (state: ProcessingState) => void;
  /** Called to check if upload was cancelled (e.g., placeholder deleted) */
  isCancelled?: () => boolean;
}

/**
 * Process and upload an image file.
 * Handles HEIC conversion, WebP conversion, thumbnails, and upload.
 * Calls onStateChange when transitioning from converting to uploading.
 * Returns the UUID of the uploaded attachment, or null if cancelled/failed.
 * Shows user-friendly error messages via toast notifications.
 */
export async function processAndUploadFile(
  file: File,
  options?: ProcessAndUploadOptions,
): Promise<string | null> {
  const { onStateChange, isCancelled } = options ?? {};

  try {
    let convertedFile: File;
    try {
      // Convert HEIC to WebP first (if needed)
      convertedFile = await convertHeicIfNeeded(file);
      // Check if cancelled during conversion
      if (isCancelled?.()) {
        return null;
      }
      // Notify that conversion is done, now processing
      if (isHeicFile(file)) {
        onStateChange?.("pending");
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
      const uuid = await uploadProcessedImage(processed);
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
  const input = document.getElementById(
    "image-upload-input",
  ) as HTMLInputElement;
  if (!input) {
    throw new Error("File input element not found");
  }
  return input;
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

/**
 * Check if a placeholder exists in the document.
 */
function placeholderExists(
  view: EditorView,
  state: "converting" | "pending",
): boolean {
  const searchText =
    state === "converting"
      ? "![converting...](attachment:converting)"
      : "![uploading...](attachment:pending)";
  return view.state.doc.toString().includes(searchText);
}

/**
 * Process a single file and insert/update the placeholder in the editor.
 * Returns true if successful, false if failed/cancelled.
 */
async function uploadSingleFile(
  view: EditorView,
  file: File,
  insertPos: number,
): Promise<boolean> {
  // Use "converting" state for HEIC files, "pending" for others
  const initialState = isHeicFile(file) ? "converting" : "pending";
  const initialAlt =
    initialState === "converting" ? "converting..." : "uploading...";
  const loadingGallery = `\n::gallery{}![${initialAlt}](attachment:${initialState})::`;

  view.dispatch({
    changes: { from: insertPos, to: insertPos, insert: loadingGallery },
  });

  // Track current state for cancellation check
  let currentState: "converting" | "pending" = initialState;

  const uuid = await processAndUploadFile(file, {
    onStateChange: (newState) => {
      // Update placeholder when state changes (converting -> pending)
      if (newState === "pending" && initialState === "converting") {
        const doc = view.state.doc;
        const fullDoc = doc.toString();
        const oldText = "![converting...](attachment:converting)";
        const newText = "![uploading...](attachment:pending)";
        const placeholderIndex = fullDoc.indexOf(oldText);
        if (placeholderIndex !== -1) {
          view.dispatch({
            changes: {
              from: placeholderIndex,
              to: placeholderIndex + oldText.length,
              insert: newText,
            },
          });
        }
        currentState = "pending";
      }
    },
    isCancelled: () => !placeholderExists(view, currentState),
  });

  // uuid is null on cancel or error - clean up placeholder if it still exists
  if (uuid === null) {
    if (placeholderExists(view, currentState)) {
      const searchText =
        currentState === "converting"
          ? "::gallery{}![converting...](attachment:converting)::"
          : "::gallery{}![uploading...](attachment:pending)::";
      const fullDoc = view.state.doc.toString();
      const placeholderIndex = fullDoc.indexOf(searchText);
      if (placeholderIndex !== -1) {
        view.dispatch({
          changes: {
            from: placeholderIndex,
            to: placeholderIndex + searchText.length,
            insert: "",
          },
        });
      }
    }
    return false;
  }

  // Find and replace the placeholder with the final image
  const doc = view.state.doc;
  const searchText = "![uploading...](attachment:pending)";
  const fullDoc = doc.toString();
  const placeholderIndex = fullDoc.indexOf(searchText);

  if (placeholderIndex !== -1) {
    const newImage = `![image](attachment:${uuid})`;
    view.dispatch({
      changes: {
        from: placeholderIndex,
        to: placeholderIndex + searchText.length,
        insert: newImage,
      },
    });
    notifyAttachmentChange();
  }

  return true;
}

/**
 * Trigger an image upload via file picker.
 * Opens a file dialog, uploads the selected images one at a time, and inserts galleries on new lines below the cursor.
 * Multiple files can be selected but they are processed sequentially.
 * If one fails, continues to the next.
 */
export function triggerImageUpload(view: EditorView): void {
  // Get the end of the current line to insert after it
  const cursorPos = view.state.selection.main.head;
  const currentLine = view.state.doc.lineAt(cursorPos);
  let insertPos = currentLine.to;

  triggerFileInput(async (files) => {
    // Process files one at a time
    for (const file of files) {
      await uploadSingleFile(view, file, insertPos);
      // Update insert position to end of document for next file
      insertPos = view.state.doc.length;
    }
  });
}
