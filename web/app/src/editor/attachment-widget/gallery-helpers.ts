/**
 * Helper functions for finding and manipulating gallery positions in documents.
 */

import { GALLERY_PATTERN } from "./patterns.ts";

/** Position information for a gallery in the document */
export interface GalleryPosition {
  from: number;
  to: number;
  imagesStart: number;
  imagesEnd: number;
}

/**
 * Find the current position of a gallery by scanning for a known image UUID.
 * Returns null if not found.
 */
export function findGalleryByUuid(
  doc: { toString: () => string },
  uuid: string,
): GalleryPosition | null {
  const text = doc.toString();
  GALLERY_PATTERN.lastIndex = 0;

  let match: RegExpExecArray | null;
  while ((match = GALLERY_PATTERN.exec(text)) !== null) {
    const imagesContent = match[2]; // Group 2 is images, group 1 is config
    if (imagesContent.includes(`attachment:${uuid}`)) {
      const galleryFrom = match.index;
      const galleryTo = galleryFrom + match[0].length;
      const imagesStart = galleryFrom + match[0].indexOf(imagesContent);
      const imagesEnd = imagesStart + imagesContent.length;
      return { from: galleryFrom, to: galleryTo, imagesStart, imagesEnd };
    }
  }
  return null;
}

/**
 * Find an image position within gallery content.
 */
export function findImageInGallery(
  doc: { toString: () => string },
  uuid: string,
): { from: number; to: number } | null {
  const text = doc.toString();
  const searchPattern = new RegExp(
    `!\\[[^\\]]*\\]\\(attachment:${uuid}\\)`,
    "g",
  );
  const match = searchPattern.exec(text);
  if (match) {
    return { from: match.index, to: match.index + match[0].length };
  }
  return null;
}
