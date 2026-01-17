/**
 * Post CRUD actions.
 *
 * Handles creating new posts/folders, deleting posts, and reordering.
 */

import {
  createPost,
  deletePost,
  movePost as apiMovePost,
  type PostNode,
  reorderPosts,
} from "../api/posts.ts";
import {
  encryptPostData,
} from "../crypto/post-encryption.ts";
import {
  addPost,
  clearSaveTimeout,
  findPost,
  getEditor,
  getFirstSelectablePost,
  getLoadedPost,
  getSiblingUuids,
  movePostInSiblings,
  movePostToParent,
  removePost,
  setDecryptedTitle,
  setEditor,
  setExpanded,
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
import { selectPost } from "./selection.ts";

// Preload editor chunk - browser starts downloading immediately
const editorPromise = import("../editor/setup.ts");

/**
 * Create a new post or folder.
 */
export async function handleNewPost(
  parentId: string | null = null,
  isFolder = false,
): Promise<void> {
  // Save current post before creating new one (includes attachment refs)
  stopServerSaveInterval();
  await saveToServerNow();

  // Clear pending data
  setPendingEncryptedData(null);
  setIsDirty(false);

  try {
    const defaultTitle = isFolder ? "New Folder" : "Untitled";
    const encrypted = await encryptPostData(defaultTitle, "");

    const post = await createPost({
      title: encrypted.title,
      title_encrypted: encrypted.titleEncrypted,
      title_iv: encrypted.titleIv,
      content: encrypted.content,
      content_encrypted: encrypted.contentEncrypted,
      iv: encrypted.contentIv,
      encryption_version: encrypted.encryptionVersion,
      parent_id: parentId ?? undefined,
      is_folder: isFolder,
    });

    // For local display, use plaintext
    const displayTitle = defaultTitle;
    const displayContent = "";

    const node: PostNode = {
      uuid: post.uuid,
      title: displayTitle,
      title_encrypted: encrypted.titleEncrypted,
      title_iv: encrypted.titleIv ?? null,
      content_encrypted: encrypted.contentEncrypted,
      encryption_version: encrypted.encryptionVersion ?? null,
      position: post.position,
      parent_id: post.parent_id,
      is_folder: post.is_folder,
      has_children: false,
      children: [],
      created_at: post.created_at,
      updated_at: post.updated_at,
    };
    addPost(node, parentId);

    // Expand parent if creating under one
    if (parentId) {
      setExpanded(parentId, true);
    }

    // Set decrypted title
    setDecryptedTitle(post.uuid, displayTitle);

    // Only load into editor if it's not a folder
    if (!isFolder) {
      setLoadedPost({
        ...post,
        title: displayTitle,
        content: displayContent,
      });
      setLoadedDecryptedContent(displayContent);

      const container = document.getElementById("editor");
      if (container) {
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
      }

      const deleteBtn = document.getElementById(
        "delete-btn",
      ) as HTMLButtonElement | null;
      if (deleteBtn) {
        deleteBtn.disabled = false;
      }
    }

    renderPostList();
  } catch (err) {
    console.error("Failed to create post:", err);
  }
}

/**
 * Create a new folder.
 */
export async function handleNewFolder(
  parentId: string | null = null,
): Promise<void> {
  return handleNewPost(parentId, true);
}

/**
 * Delete the currently selected post.
 */
export async function handleDeletePost(): Promise<void> {
  const loadedPost = getLoadedPost();
  if (!loadedPost) return;

  // Check for children and show appropriate warning
  const postNode = findPost(loadedPost.uuid);
  const hasChildren = postNode?.has_children ?? false;

  const message = hasChildren
    ? "This post has nested posts that will also be deleted. Delete anyway?"
    : "Delete this post?";

  if (!confirm(message)) return;

  // Stop any pending saves
  stopServerSaveInterval();
  clearSaveTimeout();
  setPendingEncryptedData(null);
  setIsDirty(false);

  try {
    const result = await deletePost(loadedPost.uuid);

    if (result.children_deleted > 0) {
      console.log(`Deleted ${result.children_deleted} child posts`);
    }

    removePost(loadedPost.uuid);
    setLoadedPost(null);
    setLoadedDecryptedContent(null);

    const editor = getEditor();
    if (editor) {
      editor.destroy();
      setEditor(null);
    }

    renderPostList();

    // Select the first available non-folder post
    const nextPost = getFirstSelectablePost();
    if (nextPost) {
      await selectPost(nextPost);
    } else {
      // Auto-create a new post instead of showing empty state
      await handleNewPost();
    }
  } catch (err) {
    console.error("Failed to delete post:", err);
  }
}

/**
 * Handle reordering within siblings (drag and drop).
 */
export async function handleReorder(
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
): Promise<void> {
  movePostInSiblings(parentId, fromIndex, toIndex);
  renderPostList();

  try {
    await reorderPosts(parentId, getSiblingUuids(parentId));
  } catch (err) {
    console.error("Failed to save post order:", err);
  }
}

/**
 * Handle moving a post to a new parent (drag and drop).
 */
export async function handleReparent(
  uuid: string,
  newParentId: string | null,
  position: number,
): Promise<void> {
  // Get old parent ID before moving (for reordering old siblings)
  const post = findPost(uuid);
  const oldParentId = post?.parent_id ?? null;

  movePostToParent(uuid, newParentId, position);

  // If moving to a new parent, expand it
  if (newParentId) {
    setExpanded(newParentId, true);
  }

  renderPostList();

  try {
    await apiMovePost(uuid, newParentId, position);

    // Reorder old parent's siblings to fix positions
    if (oldParentId !== newParentId) {
      await reorderPosts(oldParentId, getSiblingUuids(oldParentId));
    }
  } catch (err) {
    console.error("Failed to move post:", err);
  }
}
