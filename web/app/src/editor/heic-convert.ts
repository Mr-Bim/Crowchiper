/**
 * HEIC image conversion utility.
 * Converts HEIC/HEIF images to WebP for browser compatibility.
 * Uses dynamic import to lazy-load the heic-to library only when needed.
 */

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

  // Convert HEIC to WebP
  let webpBlob: Blob;
  try {
    webpBlob = await heicTo({
      blob: file,
      type: "image/webp",
      quality: 0.85,
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
