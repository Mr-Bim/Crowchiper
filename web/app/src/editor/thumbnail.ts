/**
 * Thumbnail generation using canvas.
 * Creates JPEG thumbnails at multiple sizes for responsive display.
 */

import { convertHeicIfNeeded } from "./heic-convert.ts";

const THUMBNAIL_SIZES = {
  sm: 200, // Small - mobile
  md: 400, // Medium - tablet
  lg: 800, // Large - desktop
} as const;

const THUMBNAIL_QUALITY = 0.85;

export type ThumbnailSize = keyof typeof THUMBNAIL_SIZES;

export interface GeneratedThumbnails {
  sm: ArrayBuffer;
  md: ArrayBuffer;
  lg: ArrayBuffer;
}

/**
 * Generate thumbnails at all sizes from an image file.
 * Returns thumbnails as JPEG ArrayBuffers.
 */
export async function generateThumbnails(
  file: File,
): Promise<GeneratedThumbnails> {
  // Convert HEIC to JPEG if needed
  const convertedFile = await convertHeicIfNeeded(file);

  // Create image element from file
  const img = await loadImage(convertedFile);

  const [sm, md, lg] = await Promise.all([
    generateSingleThumbnail(img, THUMBNAIL_SIZES.sm),
    generateSingleThumbnail(img, THUMBNAIL_SIZES.md),
    generateSingleThumbnail(img, THUMBNAIL_SIZES.lg),
  ]);

  return { sm, md, lg };
}

/**
 * Generate a single thumbnail at the specified max size.
 */
async function generateSingleThumbnail(
  img: HTMLImageElement,
  maxSize: number,
): Promise<ArrayBuffer> {
  // Calculate scaled dimensions
  const { width, height } = calculateThumbnailSize(
    img.naturalWidth,
    img.naturalHeight,
    maxSize,
  );

  // Create canvas and draw scaled image
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to get canvas 2D context");
  }

  ctx.drawImage(img, 0, 0, width, height);

  // Export as WebP
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (b) => {
        if (b) {
          resolve(b);
        } else {
          reject(new Error("Failed to create thumbnail blob"));
        }
      },
      "image/webp",
      THUMBNAIL_QUALITY,
    );
  });

  return blob.arrayBuffer();
}

/**
 * Load an image from a File object.
 */
function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Failed to load image"));
    };

    img.src = url;
  });
}

/**
 * Calculate thumbnail dimensions while maintaining aspect ratio.
 * Neither dimension will exceed maxSize.
 */
function calculateThumbnailSize(
  originalWidth: number,
  originalHeight: number,
  maxSize: number,
): { width: number; height: number } {
  // If image is already small enough, keep original size
  if (originalWidth <= maxSize && originalHeight <= maxSize) {
    return { width: originalWidth, height: originalHeight };
  }

  // Scale down to fit within maxSize
  const aspectRatio = originalWidth / originalHeight;

  if (originalWidth > originalHeight) {
    // Landscape
    return {
      width: maxSize,
      height: Math.round(maxSize / aspectRatio),
    };
  } else {
    // Portrait or square
    return {
      width: Math.round(maxSize * aspectRatio),
      height: maxSize,
    };
  }
}

/**
 * Get the pixel sizes for srcset.
 */
export function getThumbnailSizes(): typeof THUMBNAIL_SIZES {
  return THUMBNAIL_SIZES;
}
