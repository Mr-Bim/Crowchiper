/**
 * Web Worker for image compression using OffscreenCanvas.
 * Enables true parallel processing of thumbnails and main image.
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
 * Process the main image - compress to fit within size limit.
 */
async function processMainImage(bitmap: ImageBitmap): Promise<ArrayBuffer> {
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to get canvas context");
  }
  ctx.drawImage(bitmap, 0, 0);

  let quality = 1.0;
  let blob: Blob | null = null;

  while (quality >= MIN_IMAGE_QUALITY) {
    blob = await canvasToBlob(canvas, quality);
    if (blob.size <= MAX_IMAGE_BYTES) {
      break;
    }
    quality -= 0.05;
  }

  if (!blob) {
    throw new Error("Failed to create image blob");
  }

  return blob.arrayBuffer();
}

/**
 * Generate a thumbnail at the specified size.
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

  let quality = startQuality;
  let blob: Blob | null = null;

  while (quality >= MIN_THUMB_QUALITY) {
    blob = await canvasToBlob(canvas, quality);
    if (blob.size <= maxBytes) {
      break;
    }
    quality -= 0.05;
  }

  return blob!.arrayBuffer();
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
