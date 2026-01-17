/**
 * Post loading and initialization.
 *
 * Handles initial loading of posts and setting up event handlers.
 */

import { listPosts } from "../api/posts.ts";
import { setOnAttachmentChange } from "../editor/attachment-widget/index.ts";
import { decryptPostTitles } from "../crypto/post-encryption.ts";
import {
  expandToDepth,
  flattenPosts,
  getFirstSelectablePost,
  setDecryptedTitles,
  setPosts,
} from "./state.ts";
import { handleSave, saveBeacon, setRenderPostList } from "./save.ts";
import {
  renderPostList,
  setReorderHandler,
  setReparentHandler,
  setSelectPostHandler,
} from "./render.ts";
import { selectPost } from "./selection.ts";
import { handleNewPost, handleReorder, handleReparent } from "./actions.ts";

/**
 * Initialize module connections to avoid circular dependencies.
 */
function initModuleConnections(): void {
  // Connect save.ts to render.ts
  setRenderPostList(renderPostList);

  // Connect render.ts to selection.ts and actions.ts
  setSelectPostHandler(selectPost);
  setReorderHandler(handleReorder);
  setReparentHandler(handleReparent);
}

/**
 * Load posts and initialize the UI.
 */
export async function loadPosts(): Promise<void> {
  try {
    // Initialize module connections
    initModuleConnections();

    // Save post and refs via beacon when page is unloading
    window.addEventListener("pagehide", saveBeacon);

    // Auto-save when attachments are uploaded or deleted
    setOnAttachmentChange(() => {
      handleSave();
    });

    const posts = await listPosts();
    setPosts(posts);

    // Expand all posts to 1 levels by default
    expandToDepth(1);

    // Decrypt titles for all loaded posts
    const allPosts = flattenPosts();
    const titles = await decryptPostTitles(allPosts);
    setDecryptedTitles(titles);

    renderPostList();

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
    // Initialize module connections
    initModuleConnections();

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
  const container = document.getElementById("editor");
  if (container) {
    container.innerHTML = `<div class="empty-state">${message}</div>`;
  }
}
