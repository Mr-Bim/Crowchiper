/**
 * Upload handling for gallery widget - placeholder management and file processing.
 */

import type { EditorView } from "@codemirror/view";

import { abortUpload } from "../../shared/attachment-utils.ts";
import { mightBeHeic, showHeicConversionModal } from "../heic-convert.ts";
import { notifyAttachmentChange } from "./index.ts";
import type { UploadStage } from "./progress.ts";
import {
  isHeicFile,
  processAndUploadFile,
  triggerFileInput,
  registerUpload,
  unregisterUpload,
} from "./upload.ts";
import { findGalleryByUuid } from "./gallery-helpers.ts";

/** Unique ID counter for widget upload placeholders */
let widgetUploadIdCounter = 0;

/**
 * Generate a unique upload ID for tracking placeholders within widget.
 */
export function generateUploadId(): string {
  return `widget-upload-${++widgetUploadIdCounter}`;
}

/**
 * Create placeholder text for a given upload ID and stage.
 */
export function createPlaceholderText(
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
export function findWidgetPlaceholder(
  view: EditorView,
  uploadId: string,
): { from: number; to: number; text: string } | null {
  const doc = view.state.doc.toString();
  const pattern = new RegExp(
    `!\\[[^\\]]*\\]\\(attachment:${uploadId}\\)`,
    "g",
  );
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
export function updateWidgetPlaceholder(
  view: EditorView,
  uploadId: string,
  stage: UploadStage,
  percent?: number,
): boolean {
  const placeholder = findWidgetPlaceholder(view, uploadId);
  if (!placeholder) return false;

  const newText = createPlaceholderText(uploadId, stage, percent);
  if (newText !== placeholder.text) {
    view.dispatch({
      changes: {
        from: placeholder.from,
        to: placeholder.to,
        insert: newText,
      },
    });
  }
  return true;
}

/**
 * Handle cancelling an upload - aborts the upload and removes placeholder.
 */
export function handleCancelUpload(view: EditorView, uploadId: string): void {
  // Abort the upload
  abortUpload(uploadId);

  // Remove the placeholder from the document
  const placeholder = findWidgetPlaceholder(view, uploadId);
  if (placeholder) {
    view.dispatch({
      changes: {
        from: placeholder.from,
        to: placeholder.to,
        insert: "",
      },
    });

    // Clean up empty galleries
    const emptyGallery = "::gallery{}::";
    const doc = view.state.doc.toString();
    const emptyIndex = doc.indexOf(emptyGallery);
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
}

/**
 * Add a single image to an existing gallery.
 */
export async function addSingleImage(
  view: EditorView,
  file: File,
  knownUuid: string,
): Promise<void> {
  // Find current gallery position
  const gallery = findGalleryByUuid(view.state.doc, knownUuid);
  if (!gallery) {
    console.error("Could not find gallery position");
    return;
  }

  // Generate unique ID for this upload
  const uploadId = generateUploadId();

  // Create abort controller and register it
  const abortController = registerUpload(uploadId);

  // Initial stage: converting for HEIC, compressing for others
  const initialStage: UploadStage = isHeicFile(file)
    ? "converting"
    : "compressing";
  const loadingPlaceholder = createPlaceholderText(uploadId, initialStage);

  // Insert loading placeholder before the closing ::
  const insertPos = gallery.to - 2;
  view.dispatch({
    changes: { from: insertPos, to: insertPos, insert: loadingPlaceholder },
  });

  try {
    const uuid = await processAndUploadFile(file, {
      onProgress: (progress) => {
        updateWidgetPlaceholder(view, uploadId, progress.stage, progress.percent);
      },
      isCancelled: () => findWidgetPlaceholder(view, uploadId) === null,
      signal: abortController.signal,
    });

    // Unregister upload
    unregisterUpload(uploadId);

    if (uuid === null) {
      // User cancelled or placeholder deleted - clean up if still exists
      const placeholder = findWidgetPlaceholder(view, uploadId);
      if (placeholder) {
        view.dispatch({
          changes: {
            from: placeholder.from,
            to: placeholder.to,
            insert: "",
          },
        });
      }
      return;
    }

    // Replace the placeholder with the real UUID
    const placeholder = findWidgetPlaceholder(view, uploadId);
    if (placeholder) {
      const newImage = `![image](attachment:${uuid})`;
      view.dispatch({
        changes: {
          from: placeholder.from,
          to: placeholder.to,
          insert: newImage,
        },
      });
      notifyAttachmentChange();
    }
  } catch {
    // Unregister upload on error
    unregisterUpload(uploadId);

    // Remove the placeholder on error if it still exists
    const placeholder = findWidgetPlaceholder(view, uploadId);
    if (placeholder) {
      view.dispatch({
        changes: {
          from: placeholder.from,
          to: placeholder.to,
          insert: "",
        },
      });
    }
  }
}

/**
 * Trigger file input and add selected images to an existing gallery.
 */
export function addImagesToGallery(view: EditorView, knownUuid: string): void {
  if (!knownUuid) {
    console.error("No known UUID to find gallery");
    return;
  }

  triggerFileInput(async (files) => {
    // Check for HEIC files and show warning modal
    const heicFiles = files.filter((f) => mightBeHeic(f));

    if (heicFiles.length > 0) {
      const confirmed = await showHeicConversionModal(heicFiles.length);
      if (!confirmed) {
        return; // User cancelled
      }
    }

    // Process files one at a time
    for (const file of files) {
      await addSingleImage(view, file, knownUuid);
    }
  });
}
