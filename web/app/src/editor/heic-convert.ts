/**
 * HEIC image conversion utility.
 * Converts HEIC/HEIF images to JPEG for browser compatibility.
 * Uses dynamic import to lazy-load the heic-to library only when needed.
 */

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
 * Check if a file is HEIC format and convert to JPEG if needed.
 * Returns the original file if not HEIC, or a converted JPEG File.
 * The heic-to library is only loaded when a HEIC file is detected.
 */
export async function convertHeicIfNeeded(file: File): Promise<File> {
  // Quick check based on extension/mime before loading heavy library
  if (!mightBeHeic(file)) {
    return file;
  }

  // Dynamically import heic-to only when needed
  const { isHeic, heicTo } = await import("heic-to/csp");

  // Verify it's actually a HEIC file
  const isHeicFile = await isHeic(file);
  if (!isHeicFile) {
    return file;
  }

  // Convert HEIC to WebP
  const webpBlob = await heicTo({
    blob: file,
    type: "image/webp",
    quality: 0.92,
  });

  // Create a new File with .webp extension
  const newName = file.name.replace(/\.heic$/i, ".webp");
  return new File([webpBlob], newName, { type: "image/webp" });
}
