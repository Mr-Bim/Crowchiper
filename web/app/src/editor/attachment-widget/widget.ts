/**
 * Gallery container widget for CodeMirror.
 * Displays images with thumbnails, delete buttons, and options panel.
 */

import { EditorView, WidgetType } from "@codemirror/view";

import { notifyAttachmentChange } from "./index.ts";
import { findGalleryByUuid, findImageInGallery } from "./gallery-helpers.ts";
import { showFullImage } from "./lightbox.ts";
import { renderImage, type ThumbnailRenderContext } from "./thumbnail.ts";
import { addImagesToGallery, handleCancelUpload } from "./widget-upload.ts";
import type { GalleryImage } from "./types.ts";

// Re-export for backward compatibility
export type { GalleryImage } from "./types.ts";

/** Widget for the gallery container with images and options panel */
export class GalleryContainerWidget extends WidgetType {
  constructor(
    private images: GalleryImage[],
    private galleryStart: number,
    private galleryEnd: number,
  ) {
    super();
  }

  eq(other: GalleryContainerWidget): boolean {
    if (this.images.length !== other.images.length) return false;
    for (let i = 0; i < this.images.length; i++) {
      if (this.images[i].uuid !== other.images[i].uuid) return false;
      if (this.images[i].alt !== other.images[i].alt) return false;
    }
    return true;
  }

  toDOM(view: EditorView): HTMLElement {
    const container = document.createElement("div");
    container.className = "cm-gallery-container";

    // Images section
    const imagesSection = document.createElement("div");
    imagesSection.className = "cm-gallery-images";

    // Create render context for thumbnail rendering
    const ctx: ThumbnailRenderContext = {
      onImageClick: (uuid) => showFullImage(uuid, view),
      onDeleteImage: (img) => this.deleteImage(view, img),
      onCancelUpload: (uploadId) => handleCancelUpload(view, uploadId),
      view,
    };

    for (const img of this.images) {
      const imageWrapper = document.createElement("span");
      imageWrapper.className = "cm-gallery-image";
      renderImage(imageWrapper, img, ctx);
      imagesSection.appendChild(imageWrapper);
    }

    container.appendChild(imagesSection);

    // Options panel on the right
    const optionsPanel = document.createElement("div");
    optionsPanel.className = "cm-gallery-options";

    // Add image button
    const addBtn = document.createElement("button");
    addBtn.type = "button";
    addBtn.className = "cm-gallery-option-btn";
    addBtn.setAttribute("aria-label", "Add image");
    addBtn.setAttribute("title", "Add image");
    addBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>`;
    addBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.addImage(view);
    });
    optionsPanel.appendChild(addBtn);

    // Delete gallery button
    const deleteBtn = document.createElement("button");
    deleteBtn.type = "button";
    deleteBtn.className = "cm-gallery-option-btn cm-gallery-option-btn-danger";
    deleteBtn.setAttribute("aria-label", "Delete gallery");
    deleteBtn.setAttribute("title", "Delete gallery");
    deleteBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>`;
    deleteBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.deleteGallery(view);
    });
    optionsPanel.appendChild(deleteBtn);

    container.appendChild(optionsPanel);

    return container;
  }

  private deleteImage(view: EditorView, img: GalleryImage): void {
    const doc = view.state.doc;

    if (this.images.length === 1) {
      // Delete the entire gallery - find it by the image UUID
      const gallery = findGalleryByUuid(doc, img.uuid);
      if (gallery) {
        view.dispatch({
          changes: { from: gallery.from, to: gallery.to, insert: "" },
        });
        notifyAttachmentChange();
      }
    } else {
      // Delete just this image - find its current position
      const imagePos = findImageInGallery(doc, img.uuid);
      if (imagePos) {
        view.dispatch({
          changes: { from: imagePos.from, to: imagePos.to, insert: "" },
        });
        notifyAttachmentChange();
      }
    }
  }

  private deleteGallery(view: EditorView): void {
    // Find the gallery by any of its image UUIDs
    const doc = view.state.doc;
    for (const img of this.images) {
      const gallery = findGalleryByUuid(doc, img.uuid);
      if (gallery) {
        view.dispatch({
          changes: { from: gallery.from, to: gallery.to, insert: "" },
        });
        notifyAttachmentChange();
        return;
      }
    }
  }

  private addImage(view: EditorView): void {
    // Capture a known UUID to find the gallery later
    const knownUuid = this.images[0]?.uuid;
    addImagesToGallery(view, knownUuid);
  }

  ignoreEvent(): boolean {
    return false;
  }
}
