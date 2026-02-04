/**
 * Post loading and initialization.
 *
 * Handles initial loading of posts and setting up event handlers.
 */

import { getPost, listPosts, listPostChildren } from "../api/posts.ts";
import { decryptPostTitles } from "../crypto/post-encryption.ts";
import { getOptionalElement } from "../../../shared/dom.ts";
import { registerHandlers, isHandlersRegistered } from "./handlers.ts";
import {
  expandToDepth,
  findPost,
  flattenPosts,
  getFirstSelectablePost,
  getLastSelectedPostUuid,
  decryptedTitlesSignal,
  setExpanded,
  postsSignal,
  setPostChildren,
} from "./state/index.ts";
import {
  scheduleAutosave,
  saveBeacon,
  setupBeforeUnloadWarning,
} from "./save.ts";
import { renderPostList } from "./render.ts";
import { selectPost } from "./selection.ts";
import { handleNewPost, handleReorder, handleReparent } from "./actions.ts";
import type { PostNode } from "../api/posts.ts";
import { decryptPostTitle } from "../crypto/post-encryption.ts";

/**
 * Expand all ancestors of a post so it's visible in the tree.
 * Loads children at each level if needed.
 */
async function expandAncestors(uuid: string): Promise<PostNode | null> {
  // First check if already in tree
  let post = findPost(uuid);
  if (post) {
    // Expand all ancestors up to root
    await expandAncestorChain(post);
    return post;
  }

  // Post not in tree yet - fetch it to get parent chain
  try {
    const fullPost = await getPost(uuid);
    if (!fullPost.parent_id) {
      // Root-level post but not in tree - might have been deleted
      return null;
    }

    // Build ancestor chain by fetching parents
    const ancestorIds: string[] = [];
    let currentParentId: string | null = fullPost.parent_id;

    while (currentParentId) {
      ancestorIds.unshift(currentParentId);
      const parent = findPost(currentParentId);
      if (parent) {
        // Found in tree, expand from here
        break;
      }
      // Need to fetch parent to continue up the chain
      const parentPost = await getPost(currentParentId);
      currentParentId = parentPost.parent_id;
    }

    // Load children at each ancestor level to reveal the path
    for (const ancestorId of ancestorIds) {
      const ancestor = findPost(ancestorId);
      if (ancestor) {
        // Load children if not already loaded
        if (ancestor.has_children && ancestor.children === null) {
          const children = await listPostChildren(ancestor.uuid);
          setPostChildren(ancestor.uuid, children);
          // Decrypt titles for newly loaded children
          for (const child of children) {
            const title = await decryptPostTitle(child);
            decryptedTitlesSignal.update((m) =>
              new Map(m).set(child.uuid, title),
            );
          }
        }
        setExpanded(ancestor.uuid, true);
      }
    }

    // Now the post should be in the tree
    post = findPost(uuid);
    if (post) {
      await expandAncestorChain(post);
      return post;
    }
  } catch {
    // Post might have been deleted
    return null;
  }

  return null;
}

/**
 * Expand all ancestors in the tree for a post that's already loaded.
 */
async function expandAncestorChain(post: PostNode): Promise<void> {
  let currentParentId = post.parent_id;
  while (currentParentId) {
    const parent = findPost(currentParentId);
    if (!parent) break;

    // Load children if needed
    if (parent.has_children && parent.children === null) {
      const children = await listPostChildren(parent.uuid);
      setPostChildren(parent.uuid, children);
      // Decrypt titles for newly loaded children
      for (const child of children) {
        const title = await decryptPostTitle(child);
        decryptedTitlesSignal.update((m) => new Map(m).set(child.uuid, title));
      }
    }
    setExpanded(parent.uuid, true);
    currentParentId = parent.parent_id;
  }
}

/**
 * Initialize the handler registry.
 * Called once during app initialization to connect modules.
 */
function initHandlers(): void {
  registerHandlers({
    selectPost,
    reorder: handleReorder,
    reparent: handleReparent,
    renderPostList,
  });
}

/**
 * Set loading state on the post list.
 */
function setPostListLoading(loading: boolean): void {
  const list = getOptionalElement("post-list");
  if (list) {
    if (loading) {
      list.setAttribute("data-loading", "");
    } else {
      list.removeAttribute("data-loading");
    }
  }
}

/**
 * Load posts and initialize the UI.
 */
export async function loadPosts(): Promise<void> {
  if (isHandlersRegistered()) {
    throw new Error("Should only be run at startup");
  }
  try {
    // Initialize handler registry
    initHandlers();

    // Save post and refs via beacon when page is unloading
    window.addEventListener("pagehide", saveBeacon);

    // Warn about unsaved changes before leaving
    setupBeforeUnloadWarning();

    // Show loading state
    setPostListLoading(true);

    // Auto-save when attachments are uploaded or deleted

    const posts = await listPosts();
    postsSignal.set(posts);

    // Expand all posts to 1 levels by default
    expandToDepth(1);

    // Decrypt titles for all loaded posts
    const allPosts = flattenPosts();
    const titles = await decryptPostTitles(allPosts);
    decryptedTitlesSignal.set(titles);

    // Clear loading state and render
    setPostListLoading(false);
    renderPostList();
    import("../editor/attachment-widget/index.ts").then((widget) =>
      widget.setOnAttachmentChange(() => {
        scheduleAutosave();
      }),
    );

    // Try to restore last selected post, or select first post
    const lastUuid = getLastSelectedPostUuid();
    let postToSelect: PostNode | null = null;

    if (lastUuid) {
      // Expand ancestors and find the post (handles lazy loading)
      postToSelect = await expandAncestors(lastUuid);
      if (postToSelect) {
        // Re-render to show expanded tree before selecting
        renderPostList();
      }
    }

    if (!postToSelect) {
      postToSelect = getFirstSelectablePost();
    }

    if (postToSelect) {
      await selectPost(postToSelect);
    } else {
      // Auto-create first post instead of showing empty state
      await handleNewPost();
    }
  } catch (err) {
    console.error("Failed to load posts:", err);
  }
}

/**
 * Load posts without selecting one (for specific use cases).
 */
export async function loadPostsWithoutSelection(): Promise<void> {
  try {
    // Initialize handler registry
    initHandlers();

    const posts = await listPosts();
    postsSignal.set(posts);

    // Expand all posts to 1 levels by default
    expandToDepth(1);

    renderPostList();

    showEmptyState("");
  } catch (err) {
    console.error("Failed to load posts:", err);
  }
}

/**
 * Show an empty state message in the editor area.
 */
function showEmptyState(message: string): void {
  const container = getOptionalElement("editor");
  if (container) {
    container.innerHTML = `<div class="empty-state">${message}</div>`;
  }
}
