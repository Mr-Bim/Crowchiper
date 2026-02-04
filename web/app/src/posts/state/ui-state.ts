/**
 * Non-reactive UI state for posts.
 *
 * State that affects rendering but doesn't need reactive subscriptions.
 * Changes to this state require manual re-render calls.
 */

import type { PostNode } from "../../api/posts.ts";
import {
  postsSignal,
  expandedPostsSignal,
  expandedChangedSignal,
} from "./signals.ts";

// Re-export for convenience
export type { PendingEncryptedData } from "../types.ts";

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

// --- Expanded Posts Helper Functions ---

export function isExpanded(uuid: string): boolean {
  return expandedPostsSignal.get().has(uuid);
}

export function toggleExpanded(uuid: string): void {
  const expanded = expandedPostsSignal.get();
  const newExpanded = new Set(expanded);
  if (newExpanded.has(uuid)) {
    newExpanded.delete(uuid);
    expandedPostsSignal.set(newExpanded);
    expandedChangedSignal.set({ uuid, expanded: false });
  } else {
    newExpanded.add(uuid);
    expandedPostsSignal.set(newExpanded);
    expandedChangedSignal.set({ uuid, expanded: true });
  }
}

export function setExpanded(uuid: string, expanded: boolean): void {
  const current = expandedPostsSignal.get();
  const newSet = new Set(current);
  if (expanded) {
    newSet.add(uuid);
  } else {
    newSet.delete(uuid);
  }
  expandedPostsSignal.set(newSet);
}

/**
 * Expand all posts up to a certain depth.
 */
export function expandToDepth(depth: number): void {
  const posts = postsSignal.get();
  const expanded = new Set(expandedPostsSignal.get());

  function expand(nodes: PostNode[], currentDepth: number): void {
    if (currentDepth >= depth) return;
    for (const node of nodes) {
      if (node.has_children) {
        expanded.add(node.uuid);
        if (node.children) {
          expand(node.children, currentDepth + 1);
        }
      }
    }
  }
  expand(posts, 0);
  expandedPostsSignal.set(expanded);
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
