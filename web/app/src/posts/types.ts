/**
 * Type definitions for the posts module.
 *
 * Formalizes contracts between modules to ensure type safety.
 */

import type { Post, PostNode } from "../api/posts.ts";
import type { EditorView } from "../editor/setup.ts";

// --- State Shape ---

/**
 * Encrypted data that has been encrypted locally but not yet saved to the server.
 */
export interface PendingEncryptedData {
  title: string;
  titleEncrypted: boolean;
  titleIv: string | null;
  content: string;
  contentEncrypted: boolean;
  contentIv: string | null;
  encryptionVersion: number | null;
}

/**
 * Complete state shape for the posts module.
 * This documents all mutable state in one place.
 */
export interface PostsState {
  /** Active CodeMirror editor instance */
  editor: EditorView | null;
  /** Tree structure of posts */
  posts: PostNode[];
  /** Currently loaded post (full data including content) */
  loadedPost: Post | null;
  /** Decrypted content of the loaded post */
  loadedDecryptedContent: string | null;
  /** Map of post UUID to decrypted title for display */
  decryptedTitles: Map<string, string>;
  /** UUIDs of expanded tree nodes */
  expandedPosts: Set<string>;
  /** Encrypted data pending server save */
  pendingEncryptedData: PendingEncryptedData | null;
  /** Whether there are unsaved changes */
  isDirty: boolean;
  /** Debounce timer for encryption */
  saveTimeout: number | null;
  /** Interval timer for periodic server saves */
  serverSaveInterval: number | null;
}

// --- Handler Types ---

/**
 * Handler for selecting a post to edit.
 */
export type SelectPostHandler = (post: PostNode) => void | Promise<void>;

/**
 * Handler for reordering posts within the same parent.
 * @param parentId - Parent UUID or null for root level
 * @param fromIndex - Original index in siblings
 * @param toIndex - Target index in siblings
 */
export type ReorderHandler = (
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
) => Promise<void>;

/**
 * Handler for moving a post to a new parent.
 * @param uuid - UUID of the post to move
 * @param newParentId - New parent UUID or null for root level
 * @param position - Position within new siblings
 */
export type ReparentHandler = (
  uuid: string,
  newParentId: string | null,
  position: number,
) => Promise<void>;

/**
 * Handler for re-rendering the post list.
 */
export type RenderPostListHandler = () => void;

/**
 * All handlers that can be registered.
 */
export interface PostHandlers {
  selectPost: SelectPostHandler;
  reorder: ReorderHandler;
  reparent: ReparentHandler;
  renderPostList: RenderPostListHandler;
}

// --- Editor Integration ---

/**
 * Callback invoked when the editor document changes.
 */
export type OnDocChangeCallback = () => void;

/**
 * Factory function to create an editor instance.
 */
export type CreateEditorFn = (
  container: HTMLElement,
  content: string,
  onDocChange: OnDocChangeCallback,
) => EditorView;

// --- Drag and Drop ---

/**
 * Data attached to a draggable post element.
 */
export interface DragData {
  uuid: string;
  parentId: string | null;
  index: number;
}

/**
 * Drop target location information.
 */
export interface DropLocation {
  uuid: string;
  parentId: string | null;
  index: number;
  edge: "top" | "bottom" | null;
}

/**
 * Result of determining where to drop.
 */
export type DropAction =
  | { type: "reorder"; parentId: string | null; fromIndex: number; toIndex: number }
  | { type: "reparent"; uuid: string; newParentId: string | null; position: number }
  | { type: "none" };

// --- Save Operations ---

/**
 * Data required to update a post on the server.
 */
export interface PostUpdatePayload {
  title: string;
  title_encrypted: boolean;
  title_iv?: string;
  content: string;
  content_encrypted: boolean;
  iv?: string;
  encryption_version?: number;
  attachment_uuids?: string[];
}
