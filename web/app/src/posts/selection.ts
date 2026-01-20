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
  parseAttachmentUuids,
} from "../editor/attachment-widget/index.ts";
import { applySpellcheckToEditor } from "../spellcheck.ts";
import {
  getEditor,
  getLoadedPost,
  setDecryptedTitle,
  setCurrentDecryptedTitle,
  setEditor,
  setIsDirty,
  setLoadedDecryptedContent,
  setLoadedPost,
  setPendingEncryptedData,
  setPreviousAttachmentUuids,
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
 * Select a post for editing.
 */
export async function selectPost(postNode: PostNode): Promise<void> {
  // Save current post to server before switching (includes attachment refs)
  stopServerSaveInterval();
  await saveToServerNow();

  // Clear pending data for new post
  setPendingEncryptedData(null);
  setIsDirty(false);

  const container = document.getElementById("editor");
  if (!container) return;

  // Fetch full post data
  const post = await getPost(postNode.uuid);
  setLoadedPost(post);

  // Decrypt content and clean up any interrupted upload placeholders
  const decryptedContent = await decryptPostContent(post);
  const displayContent = cleanupPendingUploads(decryptedContent);
  setLoadedDecryptedContent(displayContent);

  // Track initial attachment UUIDs for this post
  setPreviousAttachmentUuids(parseAttachmentUuids(displayContent));

  // Decrypt title for display (stored separately, post.title stays encrypted)
  const displayTitle = await decryptPostTitle(post);
  setCurrentDecryptedTitle(displayTitle);
  setDecryptedTitle(post.uuid, displayTitle);

  renderPostList();

  // Destroy existing editor
  const oldEditor = getEditor();
  if (oldEditor) {
    oldEditor.destroy();
  }

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

  const deleteBtn = document.getElementById(
    "delete-btn",
  ) as HTMLButtonElement | null;
  if (deleteBtn) {
    deleteBtn.disabled = false;
  }
}
