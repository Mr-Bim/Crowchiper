/**
 * Web Worker for image compression using OffscreenCanvas.
 * Enables true parallel processing of thumbnails and main image.
 *
 * Uses smart quality estimation and binary search for fast compression:
 * - Estimates initial quality based on image dimensions and target size
 * - Uses binary search to find optimal quality (max 4 iterations)
 * - Downscales images larger than 4K before compression
 */

/** Thumbnail size configuration */
const THUMBNAIL_SIZES = { sm: 200, md: 400, lg: 800 } as const;
const THUMBNAIL_QUALITY = { sm: 0.5, md: 0.5, lg: 0.4 } as const;
const THUMBNAIL_MAX_BYTES = {
  sm: 100 * 1024,
  md: 200 * 1024,
  lg: 400 * 1024,
} as const;

/** Main image settings */
const MAX_IMAGE_BYTES = 8 * 1024 * 1024;
const MIN_IMAGE_QUALITY = 0.5;
const MIN_THUMB_QUALITY = 0.1;

/** Maximum dimension for main images (4K) - larger images are downscaled */
const MAX_IMAGE_DIMENSION = 3840;

export type WorkerTask =
  | { type: "main"; bitmap: ImageBitmap }
  | { type: "thumbnail"; bitmap: ImageBitmap; size: "sm" | "md" | "lg" };

export type WorkerResult =
  | { type: "main"; data: ArrayBuffer }
  | { type: "thumbnail"; size: "sm" | "md" | "lg"; data: ArrayBuffer }
  | { type: "error"; message: string };

/**
 * Convert OffscreenCanvas to blob, trying WebP first.
 */
async function canvasToBlob(
  canvas: OffscreenCanvas,
  quality: number,
): Promise<Blob> {
  const blob = await canvas.convertToBlob({ type: "image/webp", quality });
  if (blob.type === "image/webp") {
    return blob;
  }
  // Fallback to JPEG if WebP not supported
  return canvas.convertToBlob({ type: "image/jpeg", quality });
}

/**
 * Estimate initial quality based on pixel count and target size.
 * Uses empirical data: WebP at quality 0.8 typically produces ~0.5 bytes/pixel.
 */
function estimateInitialQuality(
  width: number,
  height: number,
  targetBytes: number,
): number {
  const pixels = width * height;
  // Estimate bytes per pixel at quality 1.0 (roughly 0.8 bytes/pixel for WebP)
  const estimatedSize = pixels * 0.8;

  if (estimatedSize <= targetBytes) {
    return 1.0; // Image will likely fit at max quality
  }

  // Estimate quality needed: quality roughly scales linearly with output size
  const ratio = targetBytes / estimatedSize;
  // Clamp between 0.5 and 0.95 (never start below min or at absolute max)
  return Math.max(0.5, Math.min(0.95, ratio * 1.2));
}

/**
 * Find optimal quality using binary search.
 * Much faster than linear iteration - max 4 iterations instead of 10+.
 */
async function findOptimalQuality(
  canvas: OffscreenCanvas,
  targetBytes: number,
  minQuality: number,
  startQuality: number,
): Promise<Blob> {
  let low = minQuality;
  let high = startQuality;
  let bestBlob: Blob | null = null;

  // First try at estimated quality
  let blob = await canvasToBlob(canvas, high);

  // If it fits, we're done
  if (blob.size <= targetBytes) {
    return blob;
  }

  // Binary search for optimal quality (max 4 iterations)
  for (let i = 0; i < 4; i++) {
    const mid = (low + high) / 2;
    blob = await canvasToBlob(canvas, mid);

    if (blob.size <= targetBytes) {
      bestBlob = blob;
      low = mid; // Try higher quality
    } else {
      high = mid; // Need lower quality
    }

    // Stop if we're close enough (within 5% of quality range)
    if (high - low < 0.05) {
      break;
    }
  }

  // If we never found a fitting blob, use the last one at minimum quality
  if (!bestBlob) {
    bestBlob = await canvasToBlob(canvas, minQuality);
  }

  return bestBlob;
}

/**
 * Process the main image - compress to fit within size limit.
 * Downscales images larger than 4K before compression for faster processing.
 */
async function processMainImage(bitmap: ImageBitmap): Promise<ArrayBuffer> {
  let width = bitmap.width;
  let height = bitmap.height;

  // Downscale if larger than max dimension (4K)
  if (width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION) {
    const scale = MAX_IMAGE_DIMENSION / Math.max(width, height);
    width = Math.round(width * scale);
    height = Math.round(height * scale);
  }

  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to get canvas context");
  }
  ctx.drawImage(bitmap, 0, 0, width, height);

  // Estimate starting quality based on image size
  const startQuality = estimateInitialQuality(width, height, MAX_IMAGE_BYTES);

  // Use binary search to find optimal quality
  const blob = await findOptimalQuality(
    canvas,
    MAX_IMAGE_BYTES,
    MIN_IMAGE_QUALITY,
    startQuality,
  );

  return blob.arrayBuffer();
}

/**
 * Generate a thumbnail at the specified size.
 * Uses binary search for quality optimization.
 */
async function processThumbnail(
  bitmap: ImageBitmap,
  size: "sm" | "md" | "lg",
): Promise<ArrayBuffer> {
  const maxSize = THUMBNAIL_SIZES[size];
  const maxBytes = THUMBNAIL_MAX_BYTES[size];
  const startQuality = THUMBNAIL_QUALITY[size];

  // Calculate scaled dimensions
  let width = bitmap.width;
  let height = bitmap.height;

  if (width > maxSize || height > maxSize) {
    const scale = maxSize / Math.max(width, height);
    width = Math.round(width * scale);
    height = Math.round(height * scale);
  }

  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to get canvas context");
  }
  ctx.drawImage(bitmap, 0, 0, width, height);

  // Use binary search for optimal quality
  const blob = await findOptimalQuality(
    canvas,
    maxBytes,
    MIN_THUMB_QUALITY,
    startQuality,
  );

  return blob.arrayBuffer();
}

// Worker context type for postMessage with transfer
interface WorkerSelf {
  onmessage: ((e: MessageEvent<WorkerTask>) => void) | null;
  postMessage(message: unknown, options?: { transfer?: Transferable[] }): void;
}
declare const self: WorkerSelf;

self.onmessage = async (e: MessageEvent<WorkerTask>) => {
  const task = e.data;

  try {
    if (task.type === "main") {
      const data = await processMainImage(task.bitmap);
      task.bitmap.close();
      const result: WorkerResult = { type: "main", data };
      self.postMessage(result, { transfer: [data] });
    } else {
      const data = await processThumbnail(task.bitmap, task.size);
      task.bitmap.close();
      const result: WorkerResult = { type: "thumbnail", size: task.size, data };
      self.postMessage(result, { transfer: [data] });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    const result: WorkerResult = { type: "error", message };
    self.postMessage(result);
  }
};
