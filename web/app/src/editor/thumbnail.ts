/**
 * Thumbnail generation using canvas.
 * Creates WebP thumbnails at multiple sizes for responsive display.
 */

import { convertHeicIfNeeded } from "./heic-convert.ts";

const THUMBNAIL_SIZES = {
  sm: 200, // Small - mobile
  md: 400, // Medium - tablet
  lg: 800, // Large - desktop
} as const;

const THUMBNAIL_QUALITY = 0.75;
const MIN_QUALITY = 0.2;

// Maximum sizes in bytes (matching server limits, with headroom for encryption overhead)
const MAX_SIZES_BYTES = {
  sm: 200 * 1024, // 200KB
  md: 350 * 1024, // 350KB
  lg: 700 * 1024, // 700KB
} as const;

export type ThumbnailSize = keyof typeof THUMBNAIL_SIZES;

export interface GeneratedThumbnails {
  sm: ArrayBuffer;
  md: ArrayBuffer;
  lg: ArrayBuffer;
}

/**
 * Generate thumbnails at all sizes from an image file.
 * Returns thumbnails as WebP ArrayBuffers.
 */
export async function generateThumbnails(
  file: File,
): Promise<GeneratedThumbnails> {
  // Convert HEIC to JPEG if needed
  const convertedFile = await convertHeicIfNeeded(file);

  // Create image element from file
  const img = await loadImage(convertedFile);

  const [sm, md, lg] = await Promise.all([
    generateSingleThumbnail(img, THUMBNAIL_SIZES.sm, MAX_SIZES_BYTES.sm),
    generateSingleThumbnail(img, THUMBNAIL_SIZES.md, MAX_SIZES_BYTES.md),
    generateSingleThumbnail(img, THUMBNAIL_SIZES.lg, MAX_SIZES_BYTES.lg),
  ]);

  return { sm, md, lg };
}

/**
 * Generate a single thumbnail at the specified max size.
 * Compresses with lower quality if the result exceeds maxBytes.
 */
async function generateSingleThumbnail(
  img: HTMLImageElement,
  maxSize: number,
  maxBytes: number,
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

  // Try progressively lower quality until we fit within maxBytes
  let quality = THUMBNAIL_QUALITY;
  let blob: Blob;

  while (quality >= MIN_QUALITY) {
    blob = await canvasToBlob(canvas, quality);

    if (blob.size <= maxBytes) {
      return blob.arrayBuffer();
    }

    // Reduce quality by 10% and try again
    quality -= 0.1;
  }

  // Return the last attempt even if still over limit (best effort)
  return blob!.arrayBuffer();
}

/**
 * Convert canvas to WebP blob at specified quality.
 */
function canvasToBlob(
  canvas: HTMLCanvasElement,
  quality: number,
): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (b) => {
        if (b) {
          resolve(b);
        } else {
          reject(new Error("Failed to create thumbnail blob"));
        }
      },
      "image/webp",
      quality,
    );
  });
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
