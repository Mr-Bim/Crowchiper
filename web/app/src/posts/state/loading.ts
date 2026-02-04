/**
 * Loading state management.
 *
 * Provides a lock mechanism to prevent concurrent post operations
 * and ensure the editor isn't edited during post transitions.
 */

import { editorSignal } from "./signals.ts";

// Loading lock - prevents concurrent post selections and edits during load
let isLoadingPost = false;

/**
 * Check if a post is currently being loaded.
 */
export function isLoading(): boolean {
  return isLoadingPost;
}

/**
 * Set the loading state directly.
 * Prefer using withLoadingLock() for automatic cleanup.
 */
export function setLoading(loading: boolean): void {
  isLoadingPost = loading;

  // Update editor editability based on loading state
  const editor = editorSignal.get();
  if (editor) {
    editor.contentDOM.contentEditable = loading ? "false" : "true";
  }
}

/**
 * Execute an async function while holding the loading lock.
 * - Returns early (with undefined) if lock is already held
 * - Automatically makes editor read-only during execution
 * - Always releases lock and restores editor editability on completion
 *
 * @returns The result of the function, or undefined if lock was already held
 */
export async function withLoadingLock<T>(
  fn: () => Promise<T>,
): Promise<T | undefined> {
  // Return early if already loading
  if (isLoadingPost) {
    return undefined;
  }

  // Acquire lock and make editor read-only
  setLoading(true);

  try {
    return await fn();
  } finally {
    // Always release lock and restore editor
    setLoading(false);
  }
}
