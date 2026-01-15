/**
 * Gallery container widget for CodeMirror.
 * Displays images with thumbnails, delete buttons, and options panel.
 */

import { EditorView, WidgetType } from "@codemirror/view";

import {
  getAttachment,
  getAttachmentThumbnail,
  type ThumbnailSize,
} from "../../api/attachments.ts";
import { decryptBinary } from "../../crypto/operations.ts";
import { getSessionEncryptionKey } from "../../crypto/keystore.ts";

import { thumbnailCache, fullImageCache } from "./cache.ts";
import {
  isHeicFile,
  processAndUploadFile,
  triggerFileInput,
} from "./upload.ts";
import { notifyAttachmentChange } from "./index.ts";
import { GALLERY_PATTERN, GALLERY_IMAGE_PATTERN } from "./patterns.ts";

interface GalleryPosition {
  from: number;
  to: number;
  imagesStart: number;
  imagesEnd: number;
}

/**
 * Find the current position of a gallery by scanning for a known image UUID.
 * Returns null if not found.
 */
function findGalleryByUuid(
  doc: { toString: () => string },
  uuid: string,
): GalleryPosition | null {
  const text = doc.toString();
  GALLERY_PATTERN.lastIndex = 0;

  let match: RegExpExecArray | null;
  while ((match = GALLERY_PATTERN.exec(text)) !== null) {
    const imagesContent = match[2]; // Group 2 is images, group 1 is config
    if (imagesContent.includes(`attachment:${uuid}`)) {
      const galleryFrom = match.index;
      const galleryTo = galleryFrom + match[0].length;
      const imagesStart = galleryFrom + match[0].indexOf(imagesContent);
      const imagesEnd = imagesStart + imagesContent.length;
      return { from: galleryFrom, to: galleryTo, imagesStart, imagesEnd };
    }
  }
  return null;
}

/**
 * Find an image position within gallery content.
 */
function findImageInGallery(
  doc: { toString: () => string },
  uuid: string,
): { from: number; to: number } | null {
  const text = doc.toString();
  const searchPattern = new RegExp(
    `!\\[[^\\]]*\\]\\(attachment:${uuid}\\)`,
    "g",
  );
  const match = searchPattern.exec(text);
  if (match) {
    return { from: match.index, to: match.index + match[0].length };
  }
  return null;
}

/** Image data with position information */
export interface GalleryImage {
  uuid: string;
  alt: string;
  from: number;
  to: number;
}

/**
 * Determine optimal thumbnail size based on window width and device pixel ratio.
 */
function getOptimalThumbnailSize(): ThumbnailSize {
  const width = window.innerWidth;
  const height = window.innerHeight;

  if (width <= 600) return "sm";
  if (width > 1600 && height > 1600) return "lg";
  return "md";
}

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

    for (const img of this.images) {
      const imageWrapper = document.createElement("span");
      imageWrapper.className = "cm-gallery-image";
      this.renderImage(imageWrapper, img, view);
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

  private async renderImage(
    container: HTMLElement,
    img: GalleryImage,
    view: EditorView,
  ): Promise<void> {
    // Show processing states for special UUIDs
    if (img.uuid === "pending" || img.uuid === "converting") {
      const processing = document.createElement("span");
      processing.className = "cm-attachment-uploading";
      const label =
        img.uuid === "converting" ? "Converting..." : "Uploading...";
      processing.innerHTML = `<span class="cm-attachment-spinner"></span><span>${label}</span>`;
      container.appendChild(processing);
      return;
    }

    const cached = thumbnailCache.get(img.uuid);
    if (cached) {
      this.displayThumbnail(container, cached, img, view);
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
      this.displayThumbnail(container, blobUrl, img, view);
    } catch (err) {
      console.error("Failed to load thumbnail:", err);
      loading.textContent = "Failed to load";
      loading.className = "cm-attachment-error";
    }
  }

  private displayThumbnail(
    container: HTMLElement,
    src: string,
    img: GalleryImage,
    view: EditorView,
  ): void {
    const wrapper = document.createElement("span");
    wrapper.className = "cm-attachment-thumbnail-wrapper";

    const imgEl = document.createElement("img");
    imgEl.src = src;
    imgEl.alt = img.alt || "Attached image (click to enlarge)";
    imgEl.className = "cm-attachment-thumbnail";
    imgEl.title = "Click to view full size";

    imgEl.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      // Blur editor to hide mobile keyboard
      view.contentDOM.blur();
      this.showFullImage(img.uuid, view);
    });

    wrapper.appendChild(imgEl);
    container.appendChild(wrapper);

    this.addDeleteButton(container, img, view);
  }

  private addDeleteButton(
    container: HTMLElement,
    img: GalleryImage,
    view: EditorView,
  ): void {
    const deleteBtn = document.createElement("button");
    deleteBtn.type = "button";
    deleteBtn.className = "cm-gallery-delete-btn";
    deleteBtn.setAttribute("aria-label", "Delete image");
    deleteBtn.setAttribute("tabindex", "0");
    deleteBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>`;

    deleteBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.deleteImage(view, img);
    });

    deleteBtn.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        e.stopPropagation();
        this.deleteImage(view, img);
      }
    });

    container.appendChild(deleteBtn);
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
    if (!knownUuid) {
      console.error("No known UUID to find gallery");
      return;
    }

    triggerFileInput(async (files) => {
      // Process files one at a time
      for (const file of files) {
        await this.addSingleImage(view, file, knownUuid);
      }
    });
  }

  private async addSingleImage(
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

    // Use "converting" state for HEIC files, "pending" for others
    const initialState = isHeicFile(file) ? "converting" : "pending";
    const initialAlt =
      initialState === "converting" ? "converting..." : "uploading...";
    const loadingPlaceholder = `![${initialAlt}](attachment:${initialState})`;

    // Insert loading placeholder before the closing ::
    const insertPos = gallery.to - 2;
    view.dispatch({
      changes: { from: insertPos, to: insertPos, insert: loadingPlaceholder },
    });

    // Track current state for cleanup and cancellation check
    let currentState: "converting" | "pending" = initialState;

    // Helper to check if placeholder still exists
    const placeholderExists = () =>
      findImageInGallery(view.state.doc, currentState) !== null;

    try {
      const uuid = await processAndUploadFile(file, {
        onStateChange: (newState) => {
          // Update placeholder when state changes (converting -> pending)
          if (newState === "pending" && initialState === "converting") {
            const placeholderPos = findImageInGallery(
              view.state.doc,
              "converting",
            );
            if (placeholderPos) {
              view.dispatch({
                changes: {
                  from: placeholderPos.from,
                  to: placeholderPos.to,
                  insert: "![uploading...](attachment:pending)",
                },
              });
              currentState = "pending";
            }
          }
        },
        isCancelled: () => !placeholderExists(),
      });

      if (uuid === null) {
        // User cancelled or placeholder deleted - clean up if still exists
        const placeholderPos = findImageInGallery(view.state.doc, currentState);
        if (placeholderPos) {
          view.dispatch({
            changes: {
              from: placeholderPos.from,
              to: placeholderPos.to,
              insert: "",
            },
          });
        }
        return;
      }

      // Find the placeholder and replace it with the real UUID
      const placeholderPos = findImageInGallery(view.state.doc, "pending");
      if (placeholderPos) {
        const newImage = `![image](attachment:${uuid})`;
        view.dispatch({
          changes: {
            from: placeholderPos.from,
            to: placeholderPos.to,
            insert: newImage,
          },
        });
        notifyAttachmentChange();
      }
    } catch {
      // Remove the placeholder on error if it still exists
      const placeholderPos = findImageInGallery(view.state.doc, currentState);
      if (placeholderPos) {
        view.dispatch({
          changes: {
            from: placeholderPos.from,
            to: placeholderPos.to,
            insert: "",
          },
        });
      }
    }
  }

  /**
   * Get all images from all galleries in the document.
   */
  private getAllImagesFromDocument(
    view: EditorView,
  ): { uuid: string; alt: string }[] {
    const text = view.state.doc.toString();
    const images: { uuid: string; alt: string }[] = [];

    GALLERY_PATTERN.lastIndex = 0;
    let galleryMatch: RegExpExecArray | null;

    while ((galleryMatch = GALLERY_PATTERN.exec(text)) !== null) {
      const imagesContent = galleryMatch[2];
      GALLERY_IMAGE_PATTERN.lastIndex = 0;
      let imageMatch: RegExpExecArray | null;

      while (
        (imageMatch = GALLERY_IMAGE_PATTERN.exec(imagesContent)) !== null
      ) {
        const uuid = imageMatch[2];
        // Skip pending/converting placeholders
        if (uuid !== "pending" && uuid !== "converting") {
          images.push({ uuid, alt: imageMatch[1] });
        }
      }
    }

    return images;
  }

  private showFullImage(uuid: string, view: EditorView): void {
    const allImages = this.getAllImagesFromDocument(view);
    if (allImages.length === 0) return;

    let currentIndex = allImages.findIndex((img) => img.uuid === uuid);
    if (currentIndex === -1) currentIndex = 0;

    const overlay = document.createElement("div");
    overlay.className = "cm-attachment-overlay";

    // Image container for centering
    const imageContainer = document.createElement("div");
    imageContainer.className = "cm-attachment-image-container";

    // Navigation buttons (only show if multiple images)
    let prevBtn: HTMLButtonElement | null = null;
    let nextBtn: HTMLButtonElement | null = null;

    if (allImages.length > 1) {
      prevBtn = document.createElement("button");
      prevBtn.type = "button";
      prevBtn.className = "cm-attachment-nav-btn cm-attachment-nav-prev";
      prevBtn.setAttribute("aria-label", "Previous image");
      prevBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"></polyline></svg>`;

      nextBtn = document.createElement("button");
      nextBtn.type = "button";
      nextBtn.className = "cm-attachment-nav-btn cm-attachment-nav-next";
      nextBtn.setAttribute("aria-label", "Next image");
      nextBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>`;

      overlay.appendChild(prevBtn);
      overlay.appendChild(nextBtn);
    }

    // Image counter
    const counter = document.createElement("div");
    counter.className = "cm-attachment-counter";
    overlay.appendChild(counter);

    overlay.appendChild(imageContainer);
    document.body.appendChild(overlay);

    // Navigation functions
    const showImage = async (index: number) => {
      currentIndex = index;
      const img = allImages[currentIndex];

      // Update counter
      counter.textContent = `${currentIndex + 1} / ${allImages.length}`;

      // Update button visibility
      if (prevBtn) {
        prevBtn.style.visibility = currentIndex > 0 ? "visible" : "hidden";
      }
      if (nextBtn) {
        nextBtn.style.visibility =
          currentIndex < allImages.length - 1 ? "visible" : "hidden";
      }

      // Clear current content
      imageContainer.innerHTML = "";

      // Check cache first
      const cached = fullImageCache.get(img.uuid);
      if (cached) {
        const imgEl = document.createElement("img");
        imgEl.src = cached;
        imgEl.alt = img.alt || "Attached image";
        imgEl.className = "cm-attachment-full-image";
        imgEl.addEventListener("click", (e) => e.stopPropagation());
        imageContainer.appendChild(imgEl);
        return;
      }

      // Show loading
      const loading = document.createElement("div");
      loading.className = "cm-attachment-overlay-loading";
      loading.textContent = "Loading...";
      imageContainer.appendChild(loading);

      try {
        const response = await getAttachment(img.uuid);
        let imageData: ArrayBuffer;

        if (response.iv) {
          const sessionEncryptionKey = getSessionEncryptionKey();
          if (!sessionEncryptionKey) {
            loading.textContent = "Unlock required";
            return;
          }
          imageData = await decryptBinary(
            response.data,
            response.iv,
            sessionEncryptionKey,
          );
        } else {
          imageData = response.data;
        }

        const blob = new Blob([imageData], { type: "image/webp" });
        const blobUrl = URL.createObjectURL(blob);
        fullImageCache.set(img.uuid, blobUrl);

        imageContainer.innerHTML = "";
        const imgEl = document.createElement("img");
        imgEl.src = blobUrl;
        imgEl.alt = img.alt || "Attached image";
        imgEl.className = "cm-attachment-full-image";
        imgEl.addEventListener("click", (e) => e.stopPropagation());
        imageContainer.appendChild(imgEl);
      } catch (err) {
        console.error("Failed to load full image:", err);
        loading.textContent = "Failed to load";
      }
    };

    const goNext = () => {
      if (currentIndex < allImages.length - 1) {
        showImage(currentIndex + 1);
      }
    };

    const goPrev = () => {
      if (currentIndex > 0) {
        showImage(currentIndex - 1);
      }
    };

    const close = () => {
      overlay.remove();
      document.removeEventListener("keydown", keyHandler);
    };

    // Keyboard handler
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        close();
      } else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
        e.preventDefault();
        goNext();
      } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        goPrev();
      }
    };
    document.addEventListener("keydown", keyHandler);

    // Click to close (on overlay background only)
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        close();
      }
    });

    // Navigation button handlers
    if (prevBtn) {
      prevBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        goPrev();
      });
    }
    if (nextBtn) {
      nextBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        goNext();
      });
    }

    // Touch swipe support
    let touchStartX = 0;
    let touchStartY = 0;
    let touchEndX = 0;
    let touchEndY = 0;

    overlay.addEventListener(
      "touchstart",
      (e) => {
        touchStartX = e.changedTouches[0].screenX;
        touchStartY = e.changedTouches[0].screenY;
      },
      { passive: true },
    );

    overlay.addEventListener(
      "touchend",
      (e) => {
        touchEndX = e.changedTouches[0].screenX;
        touchEndY = e.changedTouches[0].screenY;
        handleSwipe();
      },
      { passive: true },
    );

    const handleSwipe = () => {
      const deltaX = touchEndX - touchStartX;
      const deltaY = touchEndY - touchStartY;
      const minSwipeDistance = 50;

      // Only handle horizontal swipes (ignore if vertical movement is larger)
      if (Math.abs(deltaX) > Math.abs(deltaY)) {
        if (deltaX < -minSwipeDistance) {
          // Swipe left -> next image
          goNext();
        } else if (deltaX > minSwipeDistance) {
          // Swipe right -> previous image
          goPrev();
        }
      }
    };

    // Show the initial image
    showImage(currentIndex);
  }

  ignoreEvent(): boolean {
    return false;
  }
}
