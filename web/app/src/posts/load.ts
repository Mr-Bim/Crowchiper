/**
 * Post loading and initialization.
 *
 * Handles initial loading of posts and setting up event handlers.
 */

import { listPosts } from "../api/posts.ts";
import { decryptPostTitles } from "../crypto/post-encryption.ts";
import { getOptionalElement } from "../../../shared/dom.ts";
import { registerHandlers } from "./handlers.ts";
import {
  expandToDepth,
  flattenPosts,
  getFirstSelectablePost,
  setDecryptedTitles,
  setPosts,
} from "./state.ts";
import { handleSave, saveBeacon, setupBeforeUnloadWarning } from "./save.ts";
import { renderPostList } from "./render.ts";
import { selectPost } from "./selection.ts";
import { handleNewPost, handleReorder, handleReparent } from "./actions.ts";

const widetPromise = import("../editor/attachment-widget/index.ts");

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
    setPosts(posts);

    // Expand all posts to 1 levels by default
    expandToDepth(1);

    // Decrypt titles for all loaded posts
    const allPosts = flattenPosts();
    const titles = await decryptPostTitles(allPosts);
    setDecryptedTitles(titles);

    // Clear loading state and render
    setPostListLoading(false);
    renderPostList();

    widetPromise.then((widet) =>
      widet.setOnAttachmentChange(() => {
        handleSave();
      }),
    );

    // Select first non-folder post
    const firstPost = getFirstSelectablePost();
    if (firstPost) {
      await selectPost(firstPost);
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
    setPosts(posts);

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
