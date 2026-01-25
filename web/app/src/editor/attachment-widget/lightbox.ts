/**
 * Full-screen image lightbox for viewing gallery images.
 */

import type { EditorView } from "@codemirror/view";

import { getAttachment } from "../../api/attachments.ts";
import { decryptBinary } from "../../crypto/operations.ts";
import { getSessionEncryptionKey } from "../../crypto/keystore.ts";

import { fullImageCache } from "./cache.ts";
import {
  sanitizeAltText,
  GALLERY_PATTERN,
  GALLERY_IMAGE_PATTERN,
} from "./patterns.ts";

import "../../../css/lightbox.css";

/**
 * Get all images from all galleries in the document.
 */
export function getAllImagesFromDocument(
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

    while ((imageMatch = GALLERY_IMAGE_PATTERN.exec(imagesContent)) !== null) {
      const uuid = imageMatch[2];
      // Skip pending/converting placeholders
      if (uuid !== "pending" && uuid !== "converting") {
        images.push({ uuid, alt: imageMatch[1] });
      }
    }
  }

  return images;
}

/**
 * Show a full-screen lightbox for viewing images.
 * Supports keyboard navigation (arrows, escape) and touch swipe.
 */
export function showFullImage(uuid: string, view: EditorView): void {
  const allImages = getAllImagesFromDocument(view);
  if (allImages.length === 0) return;

  let currentIndex = allImages.findIndex((img) => img.uuid === uuid);
  if (currentIndex === -1) currentIndex = 0;

  const overlay = document.createElement("div");
  overlay.className = "lightbox-overlay";

  // Image container for centering
  const imageContainer = document.createElement("div");
  imageContainer.className = "lightbox-image-container";

  // Navigation buttons (only show if multiple images)
  let prevBtn: HTMLButtonElement | null = null;
  let nextBtn: HTMLButtonElement | null = null;

  if (allImages.length > 1) {
    prevBtn = document.createElement("button");
    prevBtn.type = "button";
    prevBtn.className = "lightbox-nav-btn lightbox-nav-prev";
    prevBtn.setAttribute("aria-label", "Previous image");
    prevBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"></polyline></svg>`;

    nextBtn = document.createElement("button");
    nextBtn.type = "button";
    nextBtn.className = "lightbox-nav-btn lightbox-nav-next";
    nextBtn.setAttribute("aria-label", "Next image");
    nextBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>`;

    overlay.appendChild(prevBtn);
    overlay.appendChild(nextBtn);
  }

  // Image counter
  const counter = document.createElement("div");
  counter.className = "lightbox-counter";
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
      prevBtn.toggleAttribute("data-hidden", currentIndex <= 0);
    }
    if (nextBtn) {
      nextBtn.toggleAttribute(
        "data-hidden",
        currentIndex >= allImages.length - 1,
      );
    }

    // Clear current content
    imageContainer.innerHTML = "";

    // Check cache first
    const cached = fullImageCache.get(img.uuid);
    if (cached) {
      const imgEl = document.createElement("img");
      imgEl.src = cached;
      imgEl.alt = sanitizeAltText(img.alt) || "Attached image";
      imgEl.className = "lightbox-image";
      imgEl.addEventListener("click", (e) => e.stopPropagation());
      imageContainer.appendChild(imgEl);
      return;
    }

    // Show loading
    const loading = document.createElement("div");
    loading.className = "lightbox-loading";
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
      imgEl.alt = sanitizeAltText(img.alt) || "Attached image";
      imgEl.className = "lightbox-image";
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
