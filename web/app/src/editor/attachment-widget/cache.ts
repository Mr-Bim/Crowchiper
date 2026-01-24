/**
 * Re-export image cache from shared module.
 * Maintains backward compatibility for imports within the editor chunk.
 */
export {
  thumbnailCache,
  fullImageCache,
  clearImageCache,
  clearImageCacheExcept,
} from "../../shared/image-cache.ts";
