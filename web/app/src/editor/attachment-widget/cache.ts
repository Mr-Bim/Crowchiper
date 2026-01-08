/**
 * Image cache management for gallery attachments.
 * Caches decrypted thumbnail and full-size image blob URLs.
 */

// Cache for decrypted thumbnails (uuid -> blob URL)
export const thumbnailCache = new Map<string, string>();

// Cache for decrypted full images (uuid -> blob URL)
export const fullImageCache = new Map<string, string>();

/**
 * Clear all image caches.
 */
export function clearImageCache(): void {
  for (const url of thumbnailCache.values()) {
    URL.revokeObjectURL(url);
  }
  thumbnailCache.clear();

  for (const url of fullImageCache.values()) {
    URL.revokeObjectURL(url);
  }
  fullImageCache.clear();
}

/**
 * Clear cached images except for the specified UUIDs.
 */
export function clearImageCacheExcept(keepUuids: string[]): void {
  const keepSet = new Set(keepUuids);

  for (const [uuid, url] of thumbnailCache.entries()) {
    if (!keepSet.has(uuid)) {
      URL.revokeObjectURL(url);
      thumbnailCache.delete(uuid);
    }
  }

  for (const [uuid, url] of fullImageCache.entries()) {
    if (!keepSet.has(uuid)) {
      URL.revokeObjectURL(url);
      fullImageCache.delete(uuid);
    }
  }
}
