/**
 * HEIC image conversion utility.
 * Converts HEIC/HEIF images to WebP for browser compatibility.
 * Uses dynamic import to lazy-load the heic-to library only when needed.
 * Image processing uses Web Workers for true parallel compression.
 */

import type { WorkerTask, WorkerResult } from "./compress-worker.ts";

/** Custom error class for HEIC conversion failures */
export class HeicConversionError extends Error {
  constructor(
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = "HeicConversionError";
  }
}

/**
 * Check if a file might be HEIC based on extension or MIME type.
 * This is a quick check before loading the heavy library.
 */
function mightBeHeic(file: File): boolean {
  const name = file.name.toLowerCase();
  const type = file.type.toLowerCase();
  return (
    name.endsWith(".heic") ||
    name.endsWith(".heif") ||
    type === "image/heic" ||
    type === "image/heif"
  );
}

/**
 * Check if a file is HEIC format and convert to WebP if needed.
 * Returns the original file if not HEIC, or a converted WebP File.
 * The heic-to library is only loaded when a HEIC file is detected.
 * Throws HeicConversionError with user-friendly message on failure.
 */
export async function convertHeicIfNeeded(file: File): Promise<File> {
  // Quick check based on extension/mime before loading heavy library
  if (!mightBeHeic(file)) {
    return file;
  }

  // Dynamically import heic-to only when needed
  let heicModule;
  try {
    heicModule = await import("heic-to/csp");
  } catch (err) {
    throw new HeicConversionError(
      "Failed to load image converter. Please try a different image format.",
      err,
    );
  }

  const { isHeic, heicTo } = heicModule;

  // Verify it's actually a HEIC file
  let isHeicFile: boolean;
  try {
    isHeicFile = await isHeic(file);
  } catch (err) {
    throw new HeicConversionError(
      "Failed to read the image file. The file may be corrupted.",
      err,
    );
  }

  if (!isHeicFile) {
    return file;
  }

  // Convert HEIC to WebP at full quality
  let webpBlob: Blob;
  try {
    webpBlob = await heicTo({
      blob: file,
      type: "image/webp",
      quality: 1.0,
    });
  } catch (err) {
    throw new HeicConversionError(
      "Failed to convert HEIC image. Try taking a screenshot of the image instead.",
      err,
    );
  }

  // Create a new File with .webp extension
  const newName = file.name.replace(/\.heic$/i, ".webp");
  return new File([webpBlob], newName, { type: "image/webp" });
}

export interface ProcessedImage {
  image: ArrayBuffer;
  thumbnails: {
    sm: ArrayBuffer;
    md: ArrayBuffer;
    lg: ArrayBuffer;
  };
}

/** Worker pool for reuse across multiple uploads */
let workerPool: Worker[] = [];
const MAX_WORKERS = 4;

/**
 * Get or create a worker from the pool.
 */
function getWorker(): Worker {
  if (workerPool.length > 0) {
    return workerPool.pop()!;
  }
  return new Worker(new URL("./compress-worker.ts", import.meta.url), {
    type: "module",
  });
}

/**
 * Return a worker to the pool for reuse.
 */
function returnWorker(worker: Worker): void {
  if (workerPool.length < MAX_WORKERS) {
    workerPool.push(worker);
  } else {
    worker.terminate();
  }
}

/**
 * Run a task on a worker and return the result.
 */
function runWorkerTask(task: WorkerTask): Promise<WorkerResult> {
  return new Promise((resolve, reject) => {
    const worker = getWorker();

    const cleanup = () => {
      worker.onmessage = null;
      worker.onerror = null;
      returnWorker(worker);
    };

    worker.onmessage = (e: MessageEvent<WorkerResult>) => {
      cleanup();
      resolve(e.data);
    };

    worker.onerror = (e) => {
      cleanup();
      reject(new Error(e.message || "Worker error"));
    };

    // Transfer the bitmap to the worker (zero-copy)
    if (task.type === "main") {
      worker.postMessage(task, [task.bitmap]);
    } else {
      worker.postMessage(task, [task.bitmap]);
    }
  });
}

/**
 * Process an image file: convert to WebP, compress if needed, generate thumbnails.
 * Uses Web Workers for true parallel processing on multiple CPU cores.
 * HEIC files should be converted first via convertHeicIfNeeded.
 */
export async function processImage(file: File): Promise<ProcessedImage> {
  // Create ImageBitmap from file - this is transferable to workers
  const bitmap = await createImageBitmap(file);

  // Create separate bitmaps for each worker (bitmaps can only be transferred once)
  const [bitmapMain, bitmapSm, bitmapMd, bitmapLg] = await Promise.all([
    createImageBitmap(file),
    createImageBitmap(file),
    createImageBitmap(file),
    createImageBitmap(file),
  ]);

  // Close the original bitmap we used for nothing
  bitmap.close();

  // Run all tasks in parallel on separate workers
  const [mainResult, smResult, mdResult, lgResult] = await Promise.all([
    runWorkerTask({ type: "main", bitmap: bitmapMain }),
    runWorkerTask({ type: "thumbnail", bitmap: bitmapSm, size: "sm" }),
    runWorkerTask({ type: "thumbnail", bitmap: bitmapMd, size: "md" }),
    runWorkerTask({ type: "thumbnail", bitmap: bitmapLg, size: "lg" }),
  ]);

  // Check for errors
  if (mainResult.type === "error") {
    throw new Error(`Main image processing failed: ${mainResult.message}`);
  }
  if (smResult.type === "error") {
    throw new Error(`Small thumbnail failed: ${smResult.message}`);
  }
  if (mdResult.type === "error") {
    throw new Error(`Medium thumbnail failed: ${mdResult.message}`);
  }
  if (lgResult.type === "error") {
    throw new Error(`Large thumbnail failed: ${lgResult.message}`);
  }

  // Type narrowing
  if (
    mainResult.type !== "main" ||
    smResult.type !== "thumbnail" ||
    mdResult.type !== "thumbnail" ||
    lgResult.type !== "thumbnail"
  ) {
    throw new Error("Unexpected worker result type");
  }

  return {
    image: mainResult.data,
    thumbnails: {
      sm: smResult.data,
      md: mdResult.data,
      lg: lgResult.data,
    },
  };
}
