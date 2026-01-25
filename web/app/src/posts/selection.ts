/**
 * Post selection and editor lifecycle.
 *
 * Handles selecting a post for editing and managing the editor instance.
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
import { applySpellcheckToEditor } from "../spellcheck.ts";
import {
  getEditor,
  getLoadedPost,
  setDecryptedTitle,
  setEditor,
  setIsDirty,
  setLoadedDecryptedContent,
  setLoadedPost,
  setPendingEncryptedData,
} from "./state.ts";
import {
  scheduleEncrypt,
  stopServerSaveInterval,
  saveToServerNow,
} from "./save.ts";
import { renderPostList } from "./render.ts";

// Preload editor chunk - browser starts downloading immediately
const editorPromise = import("../editor/setup.ts");

/**
 * Set loading state on a post item in the sidebar.
 */
function setPostItemLoading(uuid: string, loading: boolean): void {
  const wrapper = document.querySelector(`[data-uuid="${uuid}"]`);
  const item = wrapper?.querySelector(".cl-post-item");
  const editor = getOptionalElement("editor");
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
  if (editor) {
    if (loading) {
      editor.setAttribute("aria-busy", "true");
    } else {
      editor.removeAttribute("aria-busy");
    }
  }
}

/**
 * Select a post for editing.
 */
export async function selectPost(postNode: PostNode): Promise<void> {
  // Abort any active uploads before switching
  abortAllUploads();

  // Show loading state on the post item
  setPostItemLoading(postNode.uuid, true);

  // Save current post to server before switching (includes attachment refs)
  stopServerSaveInterval();
  await saveToServerNow();

  // Clear pending data for new post
  setPendingEncryptedData(null);
  setIsDirty(false);

  const container = getOptionalElement("editor");
  if (!container) return;

  // Fetch full post data
  const post = await getPost(postNode.uuid);
  setLoadedPost(post);

  // Decrypt content and clean up any interrupted upload placeholders
  const decryptedContent = await decryptPostContent(post);
  const displayContent = cleanupPendingUploads(decryptedContent);
  setLoadedDecryptedContent(displayContent);

  // Decrypt title for display (stored separately, post.title stays encrypted)
  const displayTitle = await decryptPostTitle(post);
  setDecryptedTitle(post.uuid, displayTitle);

  renderPostList();

  // Destroy existing editor
  const oldEditor = getEditor();
  if (oldEditor) {
    oldEditor.destroy();
  }

  // Clear container (removes empty-state div or any leftover content)
  container.innerHTML = "";

  // Create new editor
  const { createEditor } = await editorPromise;
  const newEditor = createEditor(container, displayContent, () => {
    if (getLoadedPost()) {
      scheduleEncrypt();
    }
  });
  setEditor(newEditor);

  // Apply spellcheck setting to the new editor
  applySpellcheckToEditor();

  // Clear loading state
  setPostItemLoading(postNode.uuid, false);

  const deleteBtn = getOptionalElement("delete-btn", HTMLButtonElement);
  if (deleteBtn) {
    deleteBtn.disabled = false;
  }
}
