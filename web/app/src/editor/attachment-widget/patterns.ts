/**
 * Shared regex patterns for gallery parsing.
 */

/**
 * Pattern to match gallery syntax on a single line.
 * Use with line-by-line iteration (works with ^ anchor).
 * Groups: [1] = config JSON, [2] = images content
 */
export const GALLERY_LINE_PATTERN =
  /^::gallery\{([^}]*)\}((?:!\[[^\]]*\]\(attachment:[a-zA-Z0-9-]+\))+)::/g;

/**
 * Pattern to match gallery syntax anywhere in text.
 * Use when searching full document text (no ^ anchor).
 * Groups: [1] = config JSON, [2] = images content
 */
export const GALLERY_PATTERN =
  /::gallery\{([^}]*)\}((?:!\[[^\]]*\]\(attachment:[a-zA-Z0-9-]+\))+)::/g;

/**
 * Pattern for extracting individual images from gallery content.
 * Groups: [1] = alt text, [2] = UUID
 */
export const GALLERY_IMAGE_PATTERN =
  /!\[([^\]]*)\]\(attachment:([a-zA-Z0-9-]+)\)/g;

/**
 * Sanitize alt text for safe display.
 * Removes dangerous characters that could cause issues in HTML or markdown.
 */
export function sanitizeAltText(alt: string): string {
  return alt
    .replace(/[\r\n\t]/g, " ") // Replace newlines/tabs with space
    .replace(/[<>]/g, "") // Remove angle brackets (XSS prevention)
    .replace(/\s+/g, " ") // Collapse multiple spaces
    .trim()
    .slice(0, 200); // Limit length
}
