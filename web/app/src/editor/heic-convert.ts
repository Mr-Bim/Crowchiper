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

/** Error thrown when user aborts conversion */
export class HeicConversionAbortedError extends Error {
  constructor() {
    super("HEIC conversion was aborted");
    this.name = "HeicConversionAbortedError";
  }
}

/**
 * Check if a file might be HEIC based on extension or MIME type.
 * This is a quick check before loading the heavy library.
 */
export function mightBeHeic(file: File): boolean {
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
 * Show a confirmation modal for HEIC conversion.
 * Returns a promise that resolves to true if user confirms, false if cancelled.
 */
export function showHeicConversionModal(fileCount: number): Promise<boolean> {
  return new Promise((resolve) => {
    // Create modal overlay
    const overlay = document.createElement("div");
    overlay.className = "heic-modal-overlay";

    const modal = document.createElement("div");
    modal.className = "heic-modal";

    const title = document.createElement("h3");
    title.className = "heic-modal-title";
    title.textContent = "HEIC Image Detected";

    const message = document.createElement("p");
    message.className = "heic-modal-message";
    const fileText =
      fileCount === 1 ? "This image is" : `${fileCount} images are`;
    message.textContent = `${fileText} in HEIC format (Apple's image format). Converting to a web-compatible format may take 10-30 seconds per image depending on file size. The page may be less responsive during conversion.`;

    const buttonRow = document.createElement("div");
    buttonRow.className = "heic-modal-buttons";

    const cancelBtn = document.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.className = "heic-modal-btn heic-modal-btn-cancel";
    cancelBtn.textContent = "Cancel";

    const confirmBtn = document.createElement("button");
    confirmBtn.type = "button";
    confirmBtn.className = "heic-modal-btn heic-modal-btn-confirm";
    confirmBtn.textContent = "Convert";

    buttonRow.appendChild(cancelBtn);
    buttonRow.appendChild(confirmBtn);

    modal.appendChild(title);
    modal.appendChild(message);
    modal.appendChild(buttonRow);
    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    const cleanup = () => {
      overlay.remove();
      document.removeEventListener("keydown", keyHandler);
    };

    cancelBtn.addEventListener("click", () => {
      cleanup();
      resolve(false);
    });

    confirmBtn.addEventListener("click", () => {
      cleanup();
      resolve(true);
    });

    // Close on escape
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        cleanup();
        resolve(false);
      }
    };
    document.addEventListener("keydown", keyHandler);

    // Close on overlay click
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        cleanup();
        resolve(false);
      }
    });

    // Focus confirm button
    confirmBtn.focus();
  });
}

/**
 * Check if a file is HEIC format and convert to WebP if needed.
 * Returns the original file if not HEIC, or a converted WebP File.
 * The heic-to library is only loaded when a HEIC file is detected.
 * Throws HeicConversionError with user-friendly message on failure.
 * Throws HeicConversionAbortedError if aborted via signal.
 */
export async function convertHeicIfNeeded(
  file: File,
  signal?: AbortSignal,
): Promise<File> {
  // Quick check based on extension/mime before loading heavy library
  if (!mightBeHeic(file)) {
    return file;
  }

  // Check if already aborted
  if (signal?.aborted) {
    throw new HeicConversionAbortedError();
  }

  // Dynamically import heic-to only when needed
  // Use the CSP-compatible build that doesn't require unsafe-eval
  let heicModule;
  try {
    heicModule = await import("heic-to/csp");
  } catch (err) {
    throw new HeicConversionError(
      "Failed to load image converter. Please try a different image format.",
      err,
    );
  }

  // Check again after async import
  if (signal?.aborted) {
    throw new HeicConversionAbortedError();
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

  // Check again before expensive conversion
  if (signal?.aborted) {
    throw new HeicConversionAbortedError();
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
    if (signal?.aborted) {
      throw new HeicConversionAbortedError();
    }
    throw new HeicConversionError(
      "Failed to convert HEIC image. Try taking a screenshot of the image instead.",
      err,
    );
  }

  // Final abort check
  if (signal?.aborted) {
    throw new HeicConversionAbortedError();
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
