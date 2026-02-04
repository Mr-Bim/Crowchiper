/**
 * Reactive signals for posts state.
 *
 * These are the core reactive primitives that trigger UI updates when changed.
 * Import and use signals directly with .get(), .set(), .update(), and .subscribe().
 */

import type { Post, PostNode } from "../../api/posts.ts";
import type { EditorView } from "../../editor/setup.ts";
import { signal, type Signal } from "../../reactive.ts";
import type { PendingEncryptedData } from "../types.ts";

// --- Sync State Types ---

/** Sync status for autosave indicator */
export type SyncStatus = "idle" | "pending" | "syncing" | "synced" | "error";

// --- Reactive Signals ---

/** Active CodeMirror editor instance */
export const editorSignal: Signal<EditorView | null> = signal(null);

/** Tree structure of posts */
export const postsSignal: Signal<PostNode[]> = signal([]);

/** Currently loaded post (full data including content) */
export const loadedPostSignal: Signal<Post | null> = signal(null);

/** Decrypted content of the loaded post */
export const loadedDecryptedContentSignal: Signal<string | null> = signal(null);

/** Whether there are unsaved changes */
export const isDirtySignal: Signal<boolean> = signal(false);

/** Current sync status for UI indicator */
export const syncStatusSignal: Signal<SyncStatus> = signal("idle");

/** Decrypted titles for display in post list (uuid -> title) */
export const decryptedTitlesSignal: Signal<Map<string, string>> = signal(
  new Map(),
);

/** Set of expanded post UUIDs */
export const expandedPostsSignal: Signal<Set<string>> = signal(new Set());

/** Signal that emits {uuid, expanded} when a post's expanded state changes */
export const expandedChangedSignal: Signal<{
  uuid: string;
  expanded: boolean;
} | null> = signal(null);

/** Pending encrypted data awaiting server save */
export const pendingEncryptedDataSignal: Signal<PendingEncryptedData | null> =
  signal(null);
