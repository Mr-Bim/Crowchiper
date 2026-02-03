/**
 * Post selection logic.
 *
 * Handles selecting a post for editing.
 * Editor setup is delegated to editor.ts.
 */

import { getPost, type PostNode } from "../api/posts.ts";
import {
  decryptPostContent,
  decryptPostTitle,
} from "../crypto/post-encryption.ts";
import {
  cleanupPendingUploads,
  abortAllUploads,
} from "../shared/attachment-utils.ts";
import { getOptionalElement } from "../../../shared/dom.ts";
import {
  setDecryptedTitle,
  setIsDirty,
  setLastSelectedPostUuid,
  setLoadedDecryptedContent,
  setLoadedPost,
  setPendingEncryptedData,
  withLoadingLock,
} from "./state/index.ts";
import { saveToServerNow } from "./save.ts";
import { renderPostList } from "./render.ts";
import { setupEditor } from "./editor.ts";

/**
 * Set loading state on a post item in the sidebar.
 */
function setPostItemLoading(uuid: string, loading: boolean): void {
  const wrapper = document.querySelector(`[data-post-uuid="${uuid}"]`);
  const item = wrapper?.querySelector(".cl-post-item");
  const editorEl = getOptionalElement("editor");
  if (item) {
    if (loading) {
      item.setAttribute("data-loading", "");
      item.setAttribute("aria-busy", "true");
    } else {
      item.removeAttribute("data-loading");
      item.removeAttribute("aria-busy");
    }
  }
  // Also set aria-busy on the editor area during loading
  if (editorEl) {
    if (loading) {
      editorEl.setAttribute("aria-busy", "true");
    } else {
      editorEl.removeAttribute("aria-busy");
    }
  }
}

/**
 * Select a post for editing.
 * Returns early if another post is currently loading.
 */
export async function selectPost(postNode: PostNode): Promise<void> {
  await withLoadingLock(async () => {
    // Abort any active uploads before switching
    abortAllUploads();

    // Show loading state on the post item
    setPostItemLoading(postNode.uuid, true);

    // Save current post to server before switching (includes attachment refs)
    await saveToServerNow();

    // Clear pending data for new post
    setPendingEncryptedData(null);
    setIsDirty(false);

    const container = getOptionalElement("editor");
    if (!container) return;

    // Fetch full post data
    const post = await getPost(postNode.uuid);
    setLoadedPost(post);

    // Remember this post for next session
    setLastSelectedPostUuid(post.uuid);

    // Decrypt content and clean up any interrupted upload placeholders
    const decryptedContent = await decryptPostContent(post);
    const displayContent = cleanupPendingUploads(decryptedContent);
    setLoadedDecryptedContent(displayContent);

    // Decrypt title for display (stored separately, post.title stays encrypted)
    const displayTitle = await decryptPostTitle(post);
    setDecryptedTitle(post.uuid, displayTitle);

    renderPostList();

    // Set up editor with the new content (reuses existing editor if available)
    await setupEditor(container, displayContent);

    // Clear loading state
    setPostItemLoading(postNode.uuid, false);

    const deleteBtn = getOptionalElement("delete-btn", HTMLButtonElement);
    if (deleteBtn) {
      deleteBtn.disabled = false;
    }
  });
}
