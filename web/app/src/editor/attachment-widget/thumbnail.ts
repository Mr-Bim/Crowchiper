/**
 * Thumbnail rendering and display logic for gallery images.
 */

import type { EditorView } from "@codemirror/view";

import {
  getAttachmentThumbnail,
  AttachmentNotFoundError,
  type ThumbnailSize,
} from "../../api/attachments.ts";
import { decryptBinary } from "../../crypto/operations.ts";
import { getSessionEncryptionKey } from "../../crypto/keystore.ts";

import { thumbnailCache } from "./cache.ts";
import { getProgressText } from "./upload.ts";
import { showError } from "../../toast.ts";
import { sanitizeAltText } from "./patterns.ts";
import type { UploadStage } from "./progress.ts";
import type { GalleryImage } from "./types.ts";

/** Upload progress info parsed from alt text */
export interface UploadProgress {
  stage: UploadStage;
  percent?: number;
}

/**
 * Determine optimal thumbnail size based on window width and device pixel ratio.
 */
export function getOptimalThumbnailSize(): ThumbnailSize {
  const width = window.innerWidth;
  const height = window.innerHeight;

  if (width <= 600) return "sm";
  if (width > 1600 && height > 1600) return "lg";
  return "md";
}

/**
 * Check if UUID is an upload placeholder (upload-N or widget-upload-N).
 */
export function isUploadPlaceholder(uuid: string): boolean {
  return uuid.startsWith("upload-") || uuid.startsWith("widget-upload-");
}

/**
 * Parse progress info from the alt text.
 * Alt text format: "stage" or "stage:percent" (e.g., "compressing" or "uploading:45")
 */
export function parseProgressFromAlt(alt: string): UploadProgress | null {
  // Match "uploading:45" format
  const uploadMatch = alt.match(/^uploading:(\d+)$/);
  if (uploadMatch) {
    return { stage: "uploading", percent: parseInt(uploadMatch[1], 10) };
  }

  // Match simple stage names
  const stages: UploadStage[] = [
    "converting",
    "creating-thumbnails",
    "compressing",
    "encrypting",
    "uploading",
  ];
  if (stages.includes(alt as UploadStage)) {
    return { stage: alt as UploadStage };
  }

  return null;
}

/** Context for rendering thumbnails */
export interface ThumbnailRenderContext {
  onImageClick: (uuid: string) => void;
  onDeleteImage: (img: GalleryImage) => void;
  onCancelUpload: (uploadId: string) => void;
  view: EditorView;
}

/**
 * Render an image into a container element.
 * Handles upload placeholders, loading states, and cached thumbnails.
 */
export async function renderImage(
  container: HTMLElement,
  img: GalleryImage,
  ctx: ThumbnailRenderContext,
): Promise<void> {
  // Show processing states for upload placeholders
  if (isUploadPlaceholder(img.uuid)) {
    const progress = parseProgressFromAlt(img.alt);
    const processing = document.createElement("span");
    processing.className = "cm-attachment-uploading";

    // Status row with spinner and label
    const statusRow = document.createElement("span");
    statusRow.className = "cm-attachment-uploading-status";
    const label = progress ? getProgressText(progress) : "Processing...";
    statusRow.innerHTML = `<span class="cm-attachment-spinner"></span><span>${label}</span>`;
    processing.appendChild(statusRow);

    // Add cancel button inside the uploading container
    addCancelButton(processing, img.uuid, ctx.onCancelUpload);

    container.appendChild(processing);
    return;
  }

  // Legacy support for old placeholder format
  if (img.uuid === "pending" || img.uuid === "converting") {
    const processing = document.createElement("span");
    processing.className = "cm-attachment-uploading";
    const label = img.uuid === "converting" ? "Converting..." : "Uploading...";
    processing.innerHTML = `<span class="cm-attachment-spinner"></span><span>${label}</span>`;
    container.appendChild(processing);
    return;
  }

  const cached = thumbnailCache.get(img.uuid);
  if (cached) {
    displayThumbnail(container, cached, img, ctx);
    return;
  }

  const loading = document.createElement("span");
  loading.className = "cm-attachment-loading";
  loading.textContent = "Loading...";
  container.appendChild(loading);

  try {
    const response = await getAttachmentThumbnail(
      img.uuid,
      getOptimalThumbnailSize(),
    );

    let imageData: ArrayBuffer;

    // Check if data is encrypted (IV is non-empty)
    if (response.iv) {
      // Encrypted data - need to decrypt
      const sessionEncryptionKey = getSessionEncryptionKey();
      if (!sessionEncryptionKey) {
        loading.textContent = "Unlock required";
        loading.className = "cm-attachment-error";
        return;
      }
      imageData = await decryptBinary(
        response.data,
        response.iv,
        sessionEncryptionKey,
      );
    } else {
      // Unencrypted data - use directly
      imageData = response.data;
    }

    const blob = new Blob([imageData], { type: "image/webp" });
    const blobUrl = URL.createObjectURL(blob);

    thumbnailCache.set(img.uuid, blobUrl);
    container.removeChild(loading);
    displayThumbnail(container, blobUrl, img, ctx);
  } catch (err) {
    console.error("Failed to load thumbnail:", err);
    if (err instanceof AttachmentNotFoundError) {
      // Image doesn't exist on server - show toast and remove from gallery
      showError("Image not found. It may have been deleted.");
      ctx.onDeleteImage(img);
    } else {
      // Other errors (network, etc.) - show error state but don't delete
      loading.textContent = "Failed to load";
      loading.className = "cm-attachment-error";
    }
  }
}

/**
 * Display a loaded thumbnail with click and delete handlers.
 */
function displayThumbnail(
  container: HTMLElement,
  src: string,
  img: GalleryImage,
  ctx: ThumbnailRenderContext,
): void {
  const wrapper = document.createElement("span");
  wrapper.className = "cm-attachment-thumbnail-wrapper";

  const imgEl = document.createElement("img");
  imgEl.src = src;
  imgEl.alt = sanitizeAltText(img.alt) || "Attached image (click to enlarge)";
  imgEl.className = "cm-attachment-thumbnail";
  imgEl.title = "Click to view full size";

  imgEl.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    // Blur editor to hide mobile keyboard
    ctx.view.contentDOM.blur();
    ctx.onImageClick(img.uuid);
  });

  wrapper.appendChild(imgEl);
  container.appendChild(wrapper);

  addDeleteButton(container, img, ctx.onDeleteImage);
}

/**
 * Add a delete button to an image container.
 */
function addDeleteButton(
  container: HTMLElement,
  img: GalleryImage,
  onDelete: (img: GalleryImage) => void,
): void {
  const deleteBtn = document.createElement("button");
  deleteBtn.type = "button";
  deleteBtn.className = "cm-gallery-delete-btn";
  deleteBtn.setAttribute("aria-label", "Delete image");
  deleteBtn.setAttribute("tabindex", "0");
  deleteBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>`;

  deleteBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    onDelete(img);
  });

  deleteBtn.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      onDelete(img);
    }
  });

  container.appendChild(deleteBtn);
}

/**
 * Add a cancel button for upload placeholders.
 */
function addCancelButton(
  container: HTMLElement,
  uploadId: string,
  onCancel: (uploadId: string) => void,
): void {
  const cancelBtn = document.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.className = "cm-gallery-cancel-btn";
  cancelBtn.setAttribute("tabindex", "0");
  cancelBtn.textContent = "Cancel";

  cancelBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    onCancel(uploadId);
  });

  cancelBtn.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      onCancel(uploadId);
    }
  });

  container.appendChild(cancelBtn);
}
