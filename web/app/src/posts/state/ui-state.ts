/**
 * Non-reactive UI state for posts.
 *
 * State that affects rendering but doesn't need reactive subscriptions.
 * Changes to this state require manual re-render calls.
 */

import type { PostNode } from "../../api/posts.ts";
import type { PendingEncryptedData } from "../types.ts";
import { getPosts } from "./signals.ts";

// --- Last Selected Post (persisted to localStorage) ---

const LAST_POST_KEY = "crowchiper_last_post";

export function getLastSelectedPostUuid(): string | null {
  try {
    return localStorage.getItem(LAST_POST_KEY);
  } catch {
    return null;
  }
}

export function setLastSelectedPostUuid(uuid: string): void {
  try {
    localStorage.setItem(LAST_POST_KEY, uuid);
  } catch {
    // Ignore storage errors (e.g., private browsing)
  }
}

export function clearLastSelectedPostUuid(): void {
  try {
    localStorage.removeItem(LAST_POST_KEY);
  } catch {
    // Ignore storage errors
  }
}

// Re-export for convenience
export type { PendingEncryptedData } from "../types.ts";

// --- Decrypted Titles (for display in post list) ---

let decryptedTitles: Map<string, string> = new Map();

export function getDecryptedTitles(): Map<string, string> {
  return decryptedTitles;
}

export function setDecryptedTitles(titles: Map<string, string>): void {
  decryptedTitles = titles;
}

export function setDecryptedTitle(uuid: string, title: string): void {
  decryptedTitles.set(uuid, title);
}

export function getDecryptedTitle(uuid: string): string | undefined {
  return decryptedTitles.get(uuid);
}

// --- Expanded Posts (tree collapse/expand state) ---

let expandedPosts: Set<string> = new Set();

export function isExpanded(uuid: string): boolean {
  return expandedPosts.has(uuid);
}

export function toggleExpanded(uuid: string): void {
  if (expandedPosts.has(uuid)) {
    expandedPosts.delete(uuid);
  } else {
    expandedPosts.add(uuid);
  }
}

export function setExpanded(uuid: string, expanded: boolean): void {
  if (expanded) {
    expandedPosts.add(uuid);
  } else {
    expandedPosts.delete(uuid);
  }
}

/**
 * Expand all posts up to a certain depth.
 */
export function expandToDepth(depth: number): void {
  const posts = getPosts();
  function expand(nodes: PostNode[], currentDepth: number): void {
    if (currentDepth >= depth) return;
    for (const node of nodes) {
      if (node.has_children) {
        expandedPosts.add(node.uuid);
        if (node.children) {
          expand(node.children, currentDepth + 1);
        }
      }
    }
  }
  expand(posts, 0);
}

// --- Pending Encrypted Data (for save operations) ---

let pendingEncryptedData: PendingEncryptedData | null = null;

export function getPendingEncryptedData(): PendingEncryptedData | null {
  return pendingEncryptedData;
}

export function setPendingEncryptedData(
  data: PendingEncryptedData | null,
): void {
  pendingEncryptedData = data;
}

// --- Save Timers ---

let saveTimeout: number | null = null;

export function setSaveTimeout(timeout: number | null): void {
  saveTimeout = timeout;
}

export function clearSaveTimeout(): void {
  if (saveTimeout) {
    clearTimeout(saveTimeout);
    saveTimeout = null;
  }
}
