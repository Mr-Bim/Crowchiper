/**
 * Reactive signals for posts state.
 *
 * These are the core reactive primitives that trigger UI updates when changed.
 * Keep this module minimal - only signals and their direct accessors.
 */

import type { Post, PostNode } from "../../api/posts.ts";
import type { EditorView } from "../../editor/setup.ts";
import { signal, type Signal } from "../../reactive.ts";

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

// --- Simple Accessors ---

export function getEditor(): EditorView | null {
  return editorSignal.get();
}

export function setEditor(e: EditorView | null): void {
  editorSignal.set(e);
}

export function getPosts(): PostNode[] {
  return postsSignal.get();
}

export function setPosts(p: PostNode[]): void {
  postsSignal.set(p);
}

export function getLoadedPost(): Post | null {
  return loadedPostSignal.get();
}

export function setLoadedPost(post: Post | null): void {
  loadedPostSignal.set(post);
}

export function getLoadedDecryptedContent(): string | null {
  return loadedDecryptedContentSignal.get();
}

export function setLoadedDecryptedContent(content: string | null): void {
  loadedDecryptedContentSignal.set(content);
}

export function getIsDirty(): boolean {
  return isDirtySignal.get();
}

export function setIsDirty(dirty: boolean): void {
  isDirtySignal.set(dirty);
}

export function getSyncStatus(): SyncStatus {
  return syncStatusSignal.get();
}

export function setSyncStatus(status: SyncStatus): void {
  syncStatusSignal.set(status);
}
