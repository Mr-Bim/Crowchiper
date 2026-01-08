/**
 * Image upload functionality for gallery attachments.
 */

import type { EditorView } from "@codemirror/view";

import { uploadAttachment } from "../../api/attachments.ts";
import {
  ENCRYPTED_FORMAT_VERSION,
  encryptBinary,
} from "../../crypto/operations.ts";
import { getSessionEncryptionKey } from "../../crypto/keystore.ts";
import { generateThumbnails } from "../thumbnail.ts";
import { convertHeicIfNeeded } from "../heic-convert.ts";

/** Maximum file size in bytes (10 MB) */
const MAX_FILE_SIZE = 10 * 1024 * 1024;

/**
 * Processing overlay for showing upload progress stages.
 */
interface ProcessingOverlay {
  element: HTMLElement;
  updateStage: (stage: string) => void;
  close: () => void;
}

/**
 * Create and show a processing overlay with spinner and stage message.
 */
function showProcessingOverlay(initialStage: string): ProcessingOverlay {
  const overlay = document.createElement("div");
  overlay.className = "cm-processing-overlay";

  const dialog = document.createElement("div");
  dialog.className = "cm-processing-dialog";

  const spinner = document.createElement("div");
  spinner.className = "cm-processing-spinner";

  const stage = document.createElement("p");
  stage.className = "cm-processing-stage";
  stage.textContent = initialStage;

  dialog.appendChild(spinner);
  dialog.appendChild(stage);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  return {
    element: overlay,
    updateStage: (newStage: string) => {
      stage.textContent = newStage;
    },
    close: () => {
      overlay.remove();
    },
  };
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
 * Show a dialog asking the user if they want to compress a large image.
 * Returns true if user wants to compress, false if they cancel.
 */
function showCompressionDialog(fileSize: number): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "cm-upload-dialog-overlay";

    const dialog = document.createElement("div");
    dialog.className = "cm-upload-dialog";

    const title = document.createElement("h3");
    title.className = "cm-upload-dialog-title";
    title.textContent = "Image too large";

    const message = document.createElement("p");
    message.className = "cm-upload-dialog-message";
    message.textContent = `The selected image is ${formatFileSize(fileSize)}, which exceeds the maximum size of ${formatFileSize(MAX_FILE_SIZE)}. Would you like to compress it?`;

    const buttons = document.createElement("div");
    buttons.className = "cm-upload-dialog-buttons";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "cm-upload-dialog-btn cm-upload-dialog-btn-secondary";
    cancelBtn.textContent = "Cancel";
    cancelBtn.addEventListener("click", () => {
      overlay.remove();
      resolve(false);
    });

    const compressBtn = document.createElement("button");
    compressBtn.className = "cm-upload-dialog-btn cm-upload-dialog-btn-primary";
    compressBtn.textContent = "Compress";
    compressBtn.addEventListener("click", () => {
      overlay.remove();
      resolve(true);
    });

    buttons.appendChild(cancelBtn);
    buttons.appendChild(compressBtn);

    dialog.appendChild(title);
    dialog.appendChild(message);
    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    // Close on overlay click
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        overlay.remove();
        resolve(false);
      }
    });

    // Close on Escape key
    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        overlay.remove();
        resolve(false);
        document.removeEventListener("keydown", handleKeydown);
      }
    };
    document.addEventListener("keydown", handleKeydown);

    document.body.appendChild(overlay);
    compressBtn.focus();
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
          canvas.toBlob(res, "image/jpeg", quality),
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
        file.name.replace(/\.[^.]+$/, ".jpg"),
        {
          type: "image/jpeg",
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

/**
 * Upload an already-processed image file with encryption.
 * The file should already be converted from HEIC and compressed if needed.
 * Returns the UUID of the uploaded attachment.
 */
async function uploadProcessedFile(
  file: File,
  onStage?: (stage: string) => void,
): Promise<string> {
  const sessionEncryptionKey = getSessionEncryptionKey();
  if (!sessionEncryptionKey) {
    throw new Error("Encryption key not available. Please unlock first.");
  }

  onStage?.("Generating thumbnails...");
  const imageData = await file.arrayBuffer();
  const thumbnails = await generateThumbnails(file);

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
}

/**
 * Process and upload an image file.
 * Handles HEIC conversion, compression dialog, and shows processing overlay.
 * Returns the UUID of the uploaded attachment, or null if cancelled.
 */
export async function processAndUploadFile(file: File): Promise<string | null> {
  // Show processing overlay immediately
  const processing = showProcessingOverlay("Processing image...");

  try {
    // Convert HEIC to JPEG first if needed
    const isHeic =
      file.type === "image/heic" ||
      file.type === "image/heif" ||
      file.name.toLowerCase().endsWith(".heic") ||
      file.name.toLowerCase().endsWith(".heif");

    if (isHeic) {
      processing.updateStage("Converting HEIC image...");
    }

    let processedFile: File;
    try {
      processedFile = await convertHeicIfNeeded(file);
    } catch (err) {
      console.error("Failed to convert HEIC image:", err);
      processing.close();
      return null;
    }

    // Check file size and offer compression if too large
    if (processedFile.size > MAX_FILE_SIZE) {
      processing.close();
      const shouldCompress = await showCompressionDialog(processedFile.size);
      if (!shouldCompress) {
        return null;
      }
      // Re-show processing overlay for compression
      const compressingOverlay = showProcessingOverlay("Compressing image...");
      try {
        processedFile = await compressImage(processedFile);
        compressingOverlay.close();
      } catch (err) {
        console.error("Failed to compress image:", err);
        compressingOverlay.close();
        return null;
      }
      // Re-show for remaining stages
      processing.element.remove();
      Object.assign(
        processing,
        showProcessingOverlay("Generating thumbnails..."),
      );
    }

    const uuid = await uploadProcessedFile(processedFile, (stage) => {
      processing.updateStage(stage);
    });

    processing.close();
    return uuid;
  } catch (err) {
    console.error("Failed to upload image:", err);
    processing.close();
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

/**
 * Trigger an image upload via file picker.
 * Opens a file dialog, uploads the selected image, and inserts a gallery at cursor.
 */
export function triggerImageUpload(view: EditorView): void {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/*";

  input.addEventListener("change", async () => {
    const file = input.files?.[0];
    if (!file) return;

    const pos = view.state.selection.main.head;

    // Insert loading placeholder gallery
    const loadingGallery = `::gallery{}![uploading...](attachment:pending)::`;
    view.dispatch({
      changes: { from: pos, to: pos, insert: loadingGallery },
    });

    try {
      const uuid = await processAndUploadFile(file);

      if (uuid === null) {
        // User cancelled - remove placeholder
        const doc = view.state.doc;
        const fullDoc = doc.toString();
        const placeholderIndex = fullDoc.indexOf(loadingGallery);
        if (placeholderIndex !== -1) {
          view.dispatch({
            changes: {
              from: placeholderIndex,
              to: placeholderIndex + loadingGallery.length,
              insert: "",
            },
          });
        }
        return;
      }

      // Find and replace the placeholder
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
      }
    } catch {
      // Remove the placeholder gallery on error
      const doc = view.state.doc;
      const fullDoc = doc.toString();
      const placeholderIndex = fullDoc.indexOf(loadingGallery);
      if (placeholderIndex !== -1) {
        view.dispatch({
          changes: {
            from: placeholderIndex,
            to: placeholderIndex + loadingGallery.length,
            insert: "",
          },
        });
      }
    }
  });

  input.click();
}
