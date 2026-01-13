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
import { generateThumbnails } from "../thumbnail.ts";
import { convertHeicIfNeeded } from "../heic-convert.ts";

/** Maximum file size in bytes (10 MB) */
const MAX_FILE_SIZE = 10 * 1024 * 1024;

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

/** Target size for compression (8 MB to leave room for encryption overhead) */
const TARGET_COMPRESSED_SIZE = 8 * 1024 * 1024;

/**
 * Format bytes as human-readable string.
 */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Show the compression dialog asking the user if they want to compress a large image.
 * Uses the pre-existing HTML dialog element.
 * Returns true if user wants to compress, false if they cancel.
 */
function showCompressionDialog(fileSize: number): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.getElementById(
      "compress-dialog-overlay",
    ) as HTMLElement;
    const message = document.getElementById(
      "compress-dialog-message",
    ) as HTMLElement;
    const cancelBtn = document.getElementById(
      "compress-dialog-cancel",
    ) as HTMLButtonElement;
    const confirmBtn = document.getElementById(
      "compress-dialog-confirm",
    ) as HTMLButtonElement;

    message.textContent = `The selected image is ${formatFileSize(fileSize)}, which exceeds the maximum size of ${formatFileSize(MAX_FILE_SIZE)}. Would you like to compress it?`;

    const cleanup = () => {
      overlay.hidden = true;
      overlay.removeEventListener("click", handleOverlayClick);
      document.removeEventListener("keydown", handleKeydown);
      cancelBtn.removeEventListener("click", handleCancel);
      confirmBtn.removeEventListener("click", handleConfirm);
    };

    const handleCancel = () => {
      cleanup();
      resolve(false);
    };

    const handleConfirm = () => {
      cleanup();
      resolve(true);
    };

    const handleOverlayClick = (e: MouseEvent) => {
      if (e.target === overlay) {
        cleanup();
        resolve(false);
      }
    };

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        cleanup();
        resolve(false);
      }
    };

    cancelBtn.addEventListener("click", handleCancel);
    confirmBtn.addEventListener("click", handleConfirm);
    overlay.addEventListener("click", handleOverlayClick);
    document.addEventListener("keydown", handleKeydown);

    overlay.hidden = false;
    confirmBtn.focus();
  });
}

/**
 * Compress an image file to fit within the target size.
 * Uses canvas to re-encode the image with reduced quality.
 */
async function compressImage(file: File): Promise<File> {
  // Convert HEIC to JPEG first if needed
  const convertedFile = await convertHeicIfNeeded(file);

  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(convertedFile);

    img.onload = async () => {
      URL.revokeObjectURL(url);

      const canvas = document.createElement("canvas");
      let width = img.width;
      let height = img.height;

      // Scale down if very large dimensions
      const maxDimension = 4096;
      if (width > maxDimension || height > maxDimension) {
        const scale = maxDimension / Math.max(width, height);
        width = Math.round(width * scale);
        height = Math.round(height * scale);
      }

      canvas.width = width;
      canvas.height = height;

      const ctx = canvas.getContext("2d");
      if (!ctx) {
        reject(new Error("Failed to get canvas context"));
        return;
      }

      ctx.drawImage(img, 0, 0, width, height);

      // Try progressively lower quality until we fit
      let quality = 0.85;
      let blob: Blob | null = null;

      while (quality > 0.1) {
        blob = await new Promise<Blob | null>((res) =>
          canvas.toBlob(res, "image/webp", quality),
        );

        if (blob && blob.size <= TARGET_COMPRESSED_SIZE) {
          break;
        }
        quality -= 0.1;
      }

      if (!blob) {
        reject(new Error("Failed to compress image"));
        return;
      }

      const compressedFile = new File(
        [blob],
        file.name.replace(/\.[^.]+$/, ".webp"),
        {
          type: "image/webp",
        },
      );

      resolve(compressedFile);
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Failed to load image for compression"));
    };

    img.src = url;
  });
}

/** Encryption version 0 indicates no encryption */
const UNENCRYPTED_VERSION = 0;

/**
 * Upload an already-processed image file (with or without encryption).
 * The file should already be converted from HEIC and compressed if needed.
 * Returns the UUID of the uploaded attachment.
 */
async function uploadProcessedFile(
  file: File,
  onStage?: (stage: string) => void,
): Promise<string> {
  onStage?.("Generating thumbnails...");
  const imageData = await file.arrayBuffer();
  const thumbnails = await generateThumbnails(file);

  // Check if encryption is enabled
  if (isEncryptionEnabled()) {
    const sessionEncryptionKey = getSessionEncryptionKey();
    if (!sessionEncryptionKey) {
      throw new Error("Encryption key not available. Please unlock first.");
    }

    onStage?.("Encrypting and uploading...");
    const [encryptedImage, encThumbSm, encThumbMd, encThumbLg] =
      await Promise.all([
        encryptBinary(imageData, sessionEncryptionKey),
        encryptBinary(thumbnails.sm, sessionEncryptionKey),
        encryptBinary(thumbnails.md, sessionEncryptionKey),
        encryptBinary(thumbnails.lg, sessionEncryptionKey),
      ]);

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
  } else {
    // No encryption - upload raw data
    onStage?.("Uploading...");
    const response = await uploadAttachment({
      image: imageData,
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
 * Handles HEIC conversion and compression dialog.
 * Calls onStateChange when transitioning from converting to uploading.
 * Returns the UUID of the uploaded attachment, or null if cancelled.
 */
export async function processAndUploadFile(
  file: File,
  options?: ProcessAndUploadOptions,
): Promise<string | null> {
  const { onStateChange, isCancelled } = options ?? {};

  try {
    let processedFile: File;
    try {
      processedFile = await convertHeicIfNeeded(file);
      // Check if cancelled during conversion
      if (isCancelled?.()) {
        return null;
      }
      // Notify that conversion is done, now uploading
      if (isHeicFile(file)) {
        onStateChange?.("pending");
      }
    } catch (err) {
      console.error("Failed to convert HEIC image:", err);
      return null;
    }

    // Check file size and offer compression if too large
    if (processedFile.size > MAX_FILE_SIZE) {
      // Check if cancelled before showing dialog
      if (isCancelled?.()) {
        return null;
      }
      const shouldCompress = await showCompressionDialog(processedFile.size);
      if (!shouldCompress) {
        return null;
      }
      // Check if cancelled after dialog
      if (isCancelled?.()) {
        return null;
      }
      try {
        processedFile = await compressImage(processedFile);
      } catch (err) {
        console.error("Failed to compress image:", err);
        return null;
      }
    }

    // Check if cancelled before upload
    if (isCancelled?.()) {
      return null;
    }

    const uuid = await uploadProcessedFile(processedFile);
    return uuid;
  } catch (err) {
    console.error("Failed to upload image:", err);
    throw err;
  }
}

/**
 * Upload an image file with encryption (legacy function for compatibility).
 * Does NOT show processing overlay or handle HEIC/compression.
 * Use processAndUploadFile for full processing.
 */
export async function uploadImageFile(file: File): Promise<string> {
  return uploadProcessedFile(file);
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
export function triggerFileInput(handler: (file: File) => void): void {
  const input = getFileInput();

  // Remove previous handler if any
  if (currentUploadHandler) {
    input.removeEventListener("change", currentUploadHandler);
  }

  // Reset the input value so the same file can be selected again
  input.value = "";

  const wrappedHandler = () => {
    const file = input.files?.[0];
    if (file) {
      handler(file);
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
 * Trigger an image upload via file picker.
 * Opens a file dialog, uploads the selected image, and inserts a gallery at cursor.
 */
export function triggerImageUpload(view: EditorView): void {
  const pos = view.state.selection.main.head;

  triggerFileInput(async (file) => {
    // Use "converting" state for HEIC files, "pending" for others
    const initialState = isHeicFile(file) ? "converting" : "pending";
    const initialAlt =
      initialState === "converting" ? "converting..." : "uploading...";
    const loadingGallery = `::gallery{}![${initialAlt}](attachment:${initialState})::`;

    view.dispatch({
      changes: { from: pos, to: pos, insert: loadingGallery },
    });

    // Track current state for cancellation check
    let currentState: "converting" | "pending" = initialState;

    try {
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

      if (uuid === null) {
        // User cancelled or placeholder was deleted - clean up if placeholder still exists
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
        return;
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
    } catch {
      // Remove the placeholder gallery on error if it still exists
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
    }
  });
}
