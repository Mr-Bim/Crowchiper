/**
 * State management for posts and editor.
 *
 * Centralizes all mutable state for the app, now with tree structure support.
 */

import type { Post, PostNode } from "../api/posts.ts";
import type { EditorView } from "../editor/setup.ts";

let editor: EditorView | null = null;
let posts: PostNode[] = [];
let loadedPost: Post | null = null;
let loadedDecryptedContent: string | null = null;
let currentDecryptedTitle: string | null = null;
let decryptedTitles: Map<string, string> = new Map();
let saveTimeout: number | null = null;
let previousAttachmentUuids: string[] = [];

// Expanded state for tree nodes
let expandedPosts: Set<string> = new Set();

// Pending encrypted data (encrypted locally, not yet saved to server)
export interface PendingEncryptedData {
  title: string;
  titleEncrypted: boolean;
  titleIv: string | null;
  content: string;
  contentEncrypted: boolean;
  contentIv: string | null;
  encryptionVersion: number | null;
}

let pendingEncryptedData: PendingEncryptedData | null = null;

// Whether there are unsaved changes since last server sync
let isDirty = false;

// Timer for periodic server save
let serverSaveInterval: number | null = null;

// --- Editor ---

export function getEditor(): EditorView | null {
  return editor;
}

export function setEditor(e: EditorView | null): void {
  editor = e;
}

// --- Posts (Tree Structure) ---

export function getPosts(): PostNode[] {
  return posts;
}

export function setPosts(p: PostNode[]): void {
  posts = p;
}

/**
 * Find a post by UUID in the tree.
 */
export function findPost(uuid: string): PostNode | null {
  function search(nodes: PostNode[]): PostNode | null {
    for (const node of nodes) {
      if (node.uuid === uuid) return node;
      if (node.children) {
        const found = search(node.children);
        if (found) return found;
      }
    }
    return null;
  }
  return search(posts);
}

/**
 * Find the parent of a post by UUID.
 */
export function findParent(uuid: string): PostNode | null {
  function search(nodes: PostNode[], parent: PostNode | null): PostNode | null {
    for (const node of nodes) {
      if (node.uuid === uuid) return parent;
      if (node.children) {
        const found = search(node.children, node);
        if (found !== null) return found;
      }
    }
    return null;
  }
  return search(posts, null);
}

/**
 * Get the path from root to a post (ancestors).
 */
export function getPath(uuid: string): PostNode[] {
  const path: PostNode[] = [];
  function search(nodes: PostNode[]): boolean {
    for (const node of nodes) {
      if (node.uuid === uuid) {
        return true;
      }
      if (node.children) {
        if (search(node.children)) {
          path.unshift(node);
          return true;
        }
      }
    }
    return false;
  }
  search(posts);
  return path;
}

/**
 * Get siblings of a post (posts with the same parent).
 */
export function getSiblings(uuid: string): PostNode[] {
  const parent = findParent(uuid);
  if (parent && parent.children) {
    return parent.children;
  }
  // Root level
  const post = findPost(uuid);
  if (post && post.parent_id === null) {
    return posts;
  }
  return [];
}

/**
 * Add a post to the tree at the specified parent (null = root).
 */
export function addPost(post: PostNode, parentId: string | null = null): void {
  if (parentId === null) {
    posts.unshift(post);
  } else {
    const parent = findPost(parentId);
    if (parent) {
      if (!parent.children) {
        parent.children = [];
      }
      parent.children.unshift(post);
      parent.has_children = true;
    }
  }
}

/**
 * Remove a post from the tree by UUID.
 */
export function removePost(uuid: string): PostNode | null {
  function removeFromArray(nodes: PostNode[]): PostNode | null {
    const idx = nodes.findIndex((p) => p.uuid === uuid);
    if (idx !== -1) {
      const [removed] = nodes.splice(idx, 1);
      return removed;
    }
    for (const node of nodes) {
      if (node.children) {
        const removed = removeFromArray(node.children);
        if (removed) {
          if (node.children.length === 0) {
            node.has_children = false;
          }
          return removed;
        }
      }
    }
    return null;
  }
  return removeFromArray(posts);
}

/**
 * Update a post in the tree.
 */
export function updatePostInTree(
  uuid: string,
  updates: Partial<PostNode>,
): void {
  const post = findPost(uuid);
  if (post) {
    Object.assign(post, updates);
  }
}

/**
 * Move a post within its siblings (reorder).
 */
export function movePostInSiblings(
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
): void {
  const siblings =
    parentId === null ? posts : (findPost(parentId)?.children ?? []);
  if (fromIndex === toIndex) return;
  if (fromIndex < 0 || fromIndex >= siblings.length) return;
  if (toIndex < 0 || toIndex >= siblings.length) return;

  const [removed] = siblings.splice(fromIndex, 1);
  siblings.splice(toIndex, 0, removed);
}

/**
 * Move a post to a new parent at the specified position.
 */
export function movePostToParent(
  uuid: string,
  newParentId: string | null,
  position: number,
): void {
  const post = removePost(uuid);
  if (!post) return;

  post.parent_id = newParentId;

  if (newParentId === null) {
    posts.splice(position, 0, post);
  } else {
    const newParent = findPost(newParentId);
    if (newParent) {
      if (!newParent.children) {
        newParent.children = [];
      }
      newParent.children.splice(position, 0, post);
      newParent.has_children = true;
    }
  }
}

/**
 * Get UUIDs of siblings under a parent.
 */
export function getSiblingUuids(parentId: string | null): string[] {
  const siblings =
    parentId === null ? posts : (findPost(parentId)?.children ?? []);
  return siblings.map((p) => p.uuid);
}

/**
 * Set children for a post (for lazy loading).
 */
export function setPostChildren(uuid: string, children: PostNode[]): void {
  const post = findPost(uuid);
  if (post) {
    post.children = children;
    post.has_children = children.length > 0;
  }
}

/**
 * Flatten the tree to get all posts.
 */
export function flattenPosts(): PostNode[] {
  const result: PostNode[] = [];
  function flatten(nodes: PostNode[]): void {
    for (const node of nodes) {
      result.push(node);
      if (node.children) {
        flatten(node.children);
      }
    }
  }
  flatten(posts);
  return result;
}

/**
 * Get the first non-folder post in the tree (for initial selection).
 */
export function getFirstSelectablePost(): PostNode | null {
  function find(nodes: PostNode[]): PostNode | null {
    for (const node of nodes) {
      if (!node.is_folder) return node;
      if (node.children) {
        const found = find(node.children);
        if (found) return found;
      }
    }
    return null;
  }
  return find(posts);
}

// --- Expanded State ---

export function getExpandedPosts(): Set<string> {
  return expandedPosts;
}

export function setExpandedPosts(expanded: Set<string>): void {
  expandedPosts = expanded;
}

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

// --- Loaded Post ---

export function getLoadedPost(): Post | null {
  return loadedPost;
}

export function setLoadedPost(post: Post | null): void {
  loadedPost = post;
}

export function getLoadedDecryptedContent(): string | null {
  return loadedDecryptedContent;
}

export function setLoadedDecryptedContent(content: string | null): void {
  loadedDecryptedContent = content;
}

// --- Current Decrypted Title ---

export function getCurrentDecryptedTitle(): string | null {
  return currentDecryptedTitle;
}

export function setCurrentDecryptedTitle(title: string | null): void {
  currentDecryptedTitle = title;
}

// --- Decrypted Titles Map (for post list display) ---

export function getDecryptedTitles(): Map<string, string> {
  return decryptedTitles;
}

export function setDecryptedTitles(titles: Map<string, string>): void {
  decryptedTitles = titles;
}

export function setDecryptedTitle(uuid: string, title: string): void {
  decryptedTitles.set(uuid, title);
}

// --- Save Timeout ---

export function getSaveTimeout(): number | null {
  return saveTimeout;
}

export function setSaveTimeout(timeout: number | null): void {
  saveTimeout = timeout;
}

export function clearSaveTimeout(): void {
  if (saveTimeout) {
    clearTimeout(saveTimeout);
    saveTimeout = null;
  }
}

// --- Previous Attachment UUIDs ---

export function getPreviousAttachmentUuids(): string[] {
  return previousAttachmentUuids;
}

export function setPreviousAttachmentUuids(uuids: string[]): void {
  previousAttachmentUuids = uuids;
}

// --- Pending Encrypted Data ---

export function getPendingEncryptedData(): PendingEncryptedData | null {
  return pendingEncryptedData;
}

export function setPendingEncryptedData(
  data: PendingEncryptedData | null,
): void {
  pendingEncryptedData = data;
}

// --- Dirty Flag ---

export function getIsDirty(): boolean {
  return isDirty;
}

export function setIsDirty(dirty: boolean): void {
  isDirty = dirty;
}

// --- Server Save Interval ---

export function getServerSaveInterval(): number | null {
  return serverSaveInterval;
}

export function setServerSaveInterval(interval: number | null): void {
  serverSaveInterval = interval;
}

export function clearServerSaveInterval(): void {
  if (serverSaveInterval) {
    clearInterval(serverSaveInterval);
    serverSaveInterval = null;
  }
}
