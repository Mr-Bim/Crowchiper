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
import { getOptionalElement } from "../../../shared/dom.ts";
import { encryptPostData } from "../crypto/post-encryption.ts";
import {
  addPost,
  clearSaveTimeout,
  findPost,
  getFirstSelectablePost,
  getLoadedPost,
  getSiblingUuids,
  isLoading,
  movePostInSiblings,
  movePostToParent,
  removePost,
  setDecryptedTitle,
  setExpanded,
  setIsDirty,
  setLoadedDecryptedContent,
  setLoadedPost,
  setPendingEncryptedData,
} from "./state/index.ts";
import { saveToServerNow } from "./save.ts";
import { renderPostList } from "./render.ts";
import { selectPost } from "./selection.ts";
import { destroyEditor, setupEditor } from "./editor.ts";

/**
 * Create a new post.
 * Returns early if a post is currently loading.
 */
export async function handleNewPost(
  parentId: string | null = null,
): Promise<void> {
  // Ignore if currently loading a post
  if (isLoading()) return;

  // Save current post before creating new one (includes attachment refs)
  await saveToServerNow();

  // Clear pending data
  setPendingEncryptedData(null);
  setIsDirty(false);

  try {
    const defaultTitle = "Untitled";
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

    // Load into editor
    setLoadedPost({
      ...post,
      title: displayTitle,
      content: displayContent,
    });
    setLoadedDecryptedContent(displayContent);

    const container = getOptionalElement("editor");
    if (container) {
      await setupEditor(container, displayContent);
    }

    const deleteBtn = getOptionalElement("delete-btn", HTMLButtonElement);
    if (deleteBtn) {
      deleteBtn.disabled = false;
    }

    renderPostList();
  } catch (err) {
    console.error("Failed to create post:", err);
  }
}

/**
 * Delete the currently selected post.
 */
export async function handleDeletePost(): Promise<void> {
  const loadedPost = getLoadedPost();
  if (!loadedPost) return;

  await handleDeletePostByNode(findPost(loadedPost.uuid));
}

/**
 * Delete a post by its node (can be any post, not just the loaded one).
 * Returns early if a post is currently loading.
 */
export async function handleDeletePostByNode(
  postNode: PostNode | null,
): Promise<void> {
  if (!postNode) return;

  // Ignore if currently loading a post
  if (isLoading()) return;

  // Check for children and show appropriate warning
  const hasChildren = postNode.has_children ?? false;

  const message = hasChildren
    ? "This post has nested posts that will also be deleted. Delete anyway?"
    : "Delete this post?";

  if (!confirm(message)) return;

  const loadedPost = getLoadedPost();
  const isDeletingLoadedPost = loadedPost?.uuid === postNode.uuid;

  // If deleting the currently loaded post, clear pending saves
  if (isDeletingLoadedPost) {
    clearSaveTimeout();
    setPendingEncryptedData(null);
    setIsDirty(false);
  }

  try {
    const result = await deletePost(postNode.uuid);

    if (result.children_deleted > 0) {
      console.log(`Deleted ${result.children_deleted} child posts`);
    }

    removePost(postNode.uuid);

    // If we deleted the loaded post, clear the editor
    if (isDeletingLoadedPost) {
      setLoadedPost(null);
      setLoadedDecryptedContent(null);
      destroyEditor();
    }

    renderPostList();

    // If we deleted the loaded post, select another one
    if (isDeletingLoadedPost) {
      const nextPost = getFirstSelectablePost();
      if (nextPost) {
        await selectPost(nextPost);
      } else {
        // Auto-create a new post instead of showing empty state
        await handleNewPost();
      }
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
