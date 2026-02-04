/**
 * Tree operations for posts hierarchy.
 *
 * All functions that traverse or mutate the posts tree structure.
 * Uses postsSignal from signals.ts for the underlying data.
 */

import type { PostNode } from "../../api/posts.ts";
import { postsSignal } from "./signals.ts";

// --- Memoization cache for findPost ---

/** Cached UUID to PostNode lookup map */
let postLookupCache: Map<string, PostNode> | null = null;
/** Reference to the posts array used to build the cache (for invalidation) */
let cachedPostsRef: PostNode[] | null = null;

/**
 * Build or retrieve the UUID lookup cache.
 * Cache is invalidated when postsSignal changes (detected by reference check).
 */
function getPostLookupCache(): Map<string, PostNode> {
  const currentPosts = postsSignal.get();

  // Invalidate cache if posts array reference changed
  if (postLookupCache === null || cachedPostsRef !== currentPosts) {
    postLookupCache = new Map();
    cachedPostsRef = currentPosts;

    // Recursively build the lookup map
    function buildCache(nodes: PostNode[]): void {
      for (const node of nodes) {
        postLookupCache!.set(node.uuid, node);
        if (node.children) {
          buildCache(node.children);
        }
      }
    }
    buildCache(currentPosts);
  }

  return postLookupCache;
}

/**
 * Find a post by UUID in the tree.
 * Uses memoized lookup map for O(1) access instead of O(n) tree traversal.
 */
export function findPost(uuid: string): PostNode | null {
  return getPostLookupCache().get(uuid) ?? null;
}

/**
 * Find the parent of a post by UUID.
 */
export function findParent(uuid: string): PostNode | null {
  const posts = postsSignal.get();
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
 * Get siblings of a post (posts with the same parent).
 */
export function getSiblings(uuid: string): PostNode[] {
  const posts = postsSignal.get();
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
 * If afterUuid is provided, insert after that sibling; otherwise insert at beginning.
 */
export function addPost(
  post: PostNode,
  parentId: string | null = null,
  afterUuid?: string,
): void {
  const posts = postsSignal.get();

  if (parentId !== null) {
    const parent = findPost(parentId);
    if (parent && !parent.children) {
      parent.children = [];
    }
    if (parent) {
      parent.has_children = true;
    }
  }

  const targetArray = parentId === null ? posts : findPost(parentId)?.children;
  if (!targetArray) return;

  if (afterUuid) {
    const afterIndex = targetArray.findIndex((p) => p.uuid === afterUuid);
    if (afterIndex !== -1) {
      targetArray.splice(afterIndex + 1, 0, post);
    } else {
      targetArray.unshift(post);
    }
  } else {
    targetArray.unshift(post);
  }

  // Trigger reactivity by setting a new array reference
  postsSignal.set([...posts]);
}

/**
 * Remove a post from the tree by UUID.
 */
export function removePost(uuid: string): PostNode | null {
  const posts = postsSignal.get();
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
  const removed = removeFromArray(posts);
  if (removed) {
    postsSignal.set([...posts]);
  }
  return removed;
}

/**
 * Move a post within its siblings (reorder).
 */
export function movePostInSiblings(
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
): void {
  const posts = postsSignal.get();
  const siblings =
    parentId === null ? posts : (findPost(parentId)?.children ?? []);
  if (fromIndex === toIndex) return;
  if (fromIndex < 0 || fromIndex >= siblings.length) return;
  if (toIndex < 0 || toIndex >= siblings.length) return;

  const [removed] = siblings.splice(fromIndex, 1);
  siblings.splice(toIndex, 0, removed);
  postsSignal.set([...posts]);
}

/**
 * Move a post to a new parent at the specified position.
 */
export function movePostToParent(
  uuid: string,
  newParentId: string | null,
  position: number,
): void {
  const posts = postsSignal.get();
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
  postsSignal.set([...posts]);
}

/**
 * Get UUIDs of siblings under a parent.
 */
export function getSiblingUuids(parentId: string | null): string[] {
  const posts = postsSignal.get();
  const siblings =
    parentId === null ? posts : (findPost(parentId)?.children ?? []);
  return siblings.map((p: PostNode) => p.uuid);
}

/**
 * Set children for a post (for lazy loading).
 */
export function setPostChildren(uuid: string, children: PostNode[]): void {
  const post = findPost(uuid);
  if (post) {
    post.children = children;
    post.has_children = children.length > 0;
    postsSignal.set([...postsSignal.get()]);
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
  flatten(postsSignal.get());
  return result;
}

/**
 * Get the first post in the tree (for initial selection).
 */
export function getFirstSelectablePost(): PostNode | null {
  const posts = postsSignal.get();
  return posts[0] ?? null;
}
