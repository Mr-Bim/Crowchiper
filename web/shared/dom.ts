/**
 * Type-safe DOM query utilities.
 *
 * Provides null-safe element queries that throw or return null
 * instead of requiring unsafe type assertions.
 */

/**
 * Get an element by ID, throwing if not found.
 * Use when the element is required for the page to function.
 */
export function getRequiredElement<T extends HTMLElement>(
  id: string,
  type?: new () => T,
): T {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`Required element #${id} not found`);
  }
  if (type && !(el instanceof type)) {
    throw new Error(`Element #${id} is not a ${type.name}`);
  }
  return el as T;
}

/**
 * Get an element by ID, returning null if not found.
 * Use when the element is optional or conditionally present.
 */
export function getOptionalElement<T extends HTMLElement>(
  id: string,
  type?: new () => T,
): T | null {
  const el = document.getElementById(id);
  if (!el) {
    return null;
  }
  if (type && !(el instanceof type)) {
    return null;
  }
  return el as T;
}

/**
 * Escape HTML special characters to prevent XSS when inserting
 * user-controlled strings into innerHTML templates.
 */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
