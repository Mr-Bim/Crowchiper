/**
 * Posts UI module.
 *
 * Handles rendering posts, creating/editing/deleting posts,
 * and the editor lifecycle.
 */

import {
  createPost,
  deletePost,
  getPost,
  listPosts,
  type PostSummary,
  reorderPosts,
  updatePost,
  updatePostBeacon,
} from "../api/posts.ts";
import {
  clearImageCacheExcept,
  parseAttachmentUuids,
} from "../editor/attachment-widget/index.ts";
import { initDragAndDrop } from "./drag-and-drop.ts";
import {
  decryptPostContent,
  decryptPostTitle,
  decryptPostTitles,
  encryptPostData,
} from "../crypto/post-encryption.ts";
import {
  addPost,
  clearSaveTimeout,
  clearServerSaveInterval,
  getLoadedDecryptedContent,
  getLoadedPost,
  getDecryptedTitles,
  getEditor,
  getIsDirty,
  getPendingEncryptedData,
  getPosts,
  getPostUuids,
  getServerSaveInterval,
  movePost,
  removePost,
  setLoadedDecryptedContent,
  setCurrentDecryptedTitle,
  setLoadedPost,
  setDecryptedTitle,
  setDecryptedTitles,
  setEditor,
  setIsDirty,
  setPendingEncryptedData,
  setPosts,
  setPreviousAttachmentUuids,
  setSaveTimeout,
  setServerSaveInterval,
  sortPostsByPosition,
} from "./state.ts";

// Preload editor chunk - browser starts downloading immediately
const editorPromise = import("../editor/setup.ts");

// --- Title Extraction ---

function extractTitle(content: string): string {
  const firstLine = content.split("\n")[0] || "";
  const title = firstLine.replace(/^#*\s*/, "").trim();
  return title || "Untitled";
}

// --- Save Logic ---

const ENCRYPT_DEBOUNCE_MS = 1000;
const SERVER_SAVE_INTERVAL_MS = 60000;

/**
 * Schedule local encryption after 1 second of inactivity.
 */
function scheduleEncrypt(): void {
  clearSaveTimeout();
  setSaveTimeout(
    window.setTimeout(() => {
      encryptCurrentPost();
    }, ENCRYPT_DEBOUNCE_MS),
  );
}

/**
 * Encrypt the current post content locally and store in state.
 * Does NOT save to server.
 */
async function encryptCurrentPost(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  const content = editor.state.doc.toString();
  const title = extractTitle(content);

  try {
    const encrypted = await encryptPostData(title, content);

    // Store encrypted data for later server save or beacon
    setPendingEncryptedData({
      title: encrypted.title,
      titleEncrypted: encrypted.titleEncrypted,
      titleIv: encrypted.titleIv ?? null,
      content: encrypted.content,
      contentEncrypted: encrypted.contentEncrypted,
      contentIv: encrypted.contentIv ?? null,
      encryptionVersion: encrypted.encryptionVersion ?? null,
    });

    // Update decrypted title for display
    setCurrentDecryptedTitle(title);
    setDecryptedTitle(loadedPost.uuid, title);

    // Mark as dirty (needs server save)
    setIsDirty(true);

    renderPostList();

    // Start server save interval if not already running
    startServerSaveInterval();
  } catch (err) {
    console.error("Failed to encrypt:", err);
  }
}

/**
 * Start the periodic server save interval (every 60 seconds).
 */
function startServerSaveInterval(): void {
  if (getServerSaveInterval()) return; // Already running

  setServerSaveInterval(
    window.setInterval(() => {
      saveToServer();
    }, SERVER_SAVE_INTERVAL_MS),
  );
}

/**
 * Stop the periodic server save interval.
 */
function stopServerSaveInterval(): void {
  clearServerSaveInterval();
}

/**
 * Save the pending encrypted data to the server.
 */
async function saveToServer(): Promise<void> {
  const loadedPost = getLoadedPost();
  const pendingData = getPendingEncryptedData();

  if (!loadedPost || !pendingData || !getIsDirty()) return;

  try {
    await updatePost(loadedPost.uuid, {
      title: pendingData.title,
      title_encrypted: pendingData.titleEncrypted,
      title_iv: pendingData.titleIv ?? undefined,
      content: pendingData.content,
      content_encrypted: pendingData.contentEncrypted,
      iv: pendingData.contentIv ?? undefined,
      encryption_version: pendingData.encryptionVersion ?? undefined,
    });

    setIsDirty(false);
  } catch (err) {
    console.error("Failed to save to server:", err);
  }
}

/**
 * Save to server immediately when navigating away from a post.
 * Only saves if content has changed.
 */
async function saveToServerNow(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  const currentContent = editor.state.doc.toString();
  const originalContent = getLoadedDecryptedContent();

  // Only save if content has actually changed
  if (currentContent === originalContent) {
    return;
  }

  clearSaveTimeout();
  await encryptCurrentPost();

  const pendingData = getPendingEncryptedData();
  if (!pendingData) return;

  const attachmentUuids = parseAttachmentUuids(currentContent);

  try {
    await updatePost(loadedPost.uuid, {
      title: pendingData.title,
      title_encrypted: pendingData.titleEncrypted,
      title_iv: pendingData.titleIv ?? undefined,
      content: pendingData.content,
      content_encrypted: pendingData.contentEncrypted,
      iv: pendingData.contentIv ?? undefined,
      encryption_version: pendingData.encryptionVersion ?? undefined,
      attachment_uuids: attachmentUuids,
    });

    setIsDirty(false);

    // Clear cache for deleted images
    clearImageCacheExcept(attachmentUuids);
    setPreviousAttachmentUuids([]);
  } catch (err) {
    console.error("Failed to save to server:", err);
  }
}

/**
 * Save post and attachment refs via beacon when page is unloading.
 * Only saves if there's pending encrypted data (content changed since load).
 * Called from pagehide handler.
 */
export function saveBeacon(): void {
  const loadedPost = getLoadedPost();
  const editor = getEditor();
  const pendingData = getPendingEncryptedData();

  // Only send beacon if we have pending changes
  if (!loadedPost || !editor || !pendingData) return;

  const content = editor.state.doc.toString();
  const attachmentUuids = parseAttachmentUuids(content);

  updatePostBeacon(loadedPost.uuid, {
    title: pendingData.title,
    title_encrypted: pendingData.titleEncrypted,
    title_iv: pendingData.titleIv ?? undefined,
    content: pendingData.content,
    content_encrypted: pendingData.contentEncrypted,
    iv: pendingData.contentIv ?? undefined,
    encryption_version: pendingData.encryptionVersion ?? undefined,
    attachment_uuids: attachmentUuids,
  });
}

// --- Rendering ---

export function renderPostList(): void {
  const list = document.getElementById("post-list");
  if (!list) return;

  const posts = getPosts();
  const loadedPost = getLoadedPost();
  const decryptedTitles = getDecryptedTitles();

  list.innerHTML = "";

  for (let i = 0; i < posts.length; i++) {
    const post = posts[i];

    // Wrapper div for drag and drop
    const wrapper = document.createElement("div");
    wrapper.className = "post-wrapper";
    wrapper.setAttribute("data-uuid", post.uuid);
    wrapper.setAttribute("data-index", String(i));

    // Button for selection
    const item = document.createElement("button");
    item.className = "ghost post-item";
    if (loadedPost?.uuid === post.uuid) {
      item.classList.add("active");
    }
    // Use decrypted title from map, fallback to post.title, then "Untitled"
    const title = decryptedTitles.get(post.uuid) ?? post.title ?? "Untitled";
    item.textContent = title;
    item.title = title; // Show full title on hover
    item.addEventListener("click", () => selectPost(post));

    wrapper.appendChild(item);
    list.appendChild(wrapper);
  }

  // Initialize drag and drop on the list
  initDragAndDrop(list, handleReorder);
}

async function handleReorder(
  fromIndex: number,
  toIndex: number,
): Promise<void> {
  movePost(fromIndex, toIndex);
  renderPostList();

  try {
    await reorderPosts(getPostUuids());
  } catch (err) {
    console.error("Failed to save post order:", err);
  }
}

// --- Post Selection ---

export async function selectPost(postSummary: PostSummary): Promise<void> {
  // Save current post to server before switching (includes attachment refs)
  stopServerSaveInterval();
  await saveToServerNow();

  // Clear pending data for new post
  setPendingEncryptedData(null);
  setIsDirty(false);

  const container = document.getElementById("editor");
  if (!container) return;

  // Fetch full post data
  const post = await getPost(postSummary.uuid);
  setLoadedPost(post);

  // Decrypt content
  const displayContent = await decryptPostContent(post);
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

  const deleteBtn = document.getElementById(
    "delete-btn",
  ) as HTMLButtonElement | null;
  if (deleteBtn) {
    deleteBtn.disabled = false;
  }
}

// --- Post Actions ---

export async function handleNewPost(): Promise<void> {
  // Save current post before creating new one (includes attachment refs)
  stopServerSaveInterval();
  await saveToServerNow();

  // Clear pending data
  setPendingEncryptedData(null);
  setIsDirty(false);

  try {
    const encrypted = await encryptPostData("Untitled", "");

    const post = await createPost({
      title: encrypted.title,
      title_encrypted: encrypted.titleEncrypted,
      title_iv: encrypted.titleIv,
      content: encrypted.content,
      content_encrypted: encrypted.contentEncrypted,
      iv: encrypted.contentIv,
      encryption_version: encrypted.encryptionVersion,
    });

    // For local display, use plaintext
    const displayTitle = "Untitled";
    const displayContent = "";

    const summary: PostSummary = {
      uuid: post.uuid,
      title: displayTitle,
      title_encrypted: encrypted.titleEncrypted,
      title_iv: encrypted.titleIv ?? null,
      content_encrypted: encrypted.contentEncrypted,
      encryption_version: encrypted.encryptionVersion ?? null,
      position: post.position,
      created_at: post.created_at,
      updated_at: post.updated_at,
    };
    addPost(summary);

    setLoadedPost({
      ...post,
      title: displayTitle,
      content: displayContent,
    });
    setLoadedDecryptedContent(displayContent);

    renderPostList();

    const container = document.getElementById("editor");
    if (!container) return;

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

    const deleteBtn = document.getElementById(
      "delete-btn",
    ) as HTMLButtonElement | null;
    if (deleteBtn) {
      deleteBtn.disabled = false;
    }
  } catch (err) {
    console.error("Failed to create post:", err);
  }
}

export async function handleDeletePost(): Promise<void> {
  const loadedPost = getLoadedPost();
  if (!loadedPost) return;

  if (!confirm("Delete this post?")) return;

  // Stop any pending saves
  stopServerSaveInterval();
  clearSaveTimeout();
  setPendingEncryptedData(null);
  setIsDirty(false);

  try {
    await deletePost(loadedPost.uuid);

    removePost(loadedPost.uuid);
    setLoadedPost(null);
    setLoadedDecryptedContent(null);

    const editor = getEditor();
    if (editor) {
      editor.destroy();
      setEditor(null);
    }

    renderPostList();

    const posts = getPosts();
    if (posts.length > 0) {
      // Select the first remaining post
      await selectPost(posts[0]);
    } else {
      // Auto-create a new post instead of showing empty state
      await handleNewPost();
    }
  } catch (err) {
    console.error("Failed to delete post:", err);
  }
}

// --- Loading Posts ---

export async function loadPosts(): Promise<void> {
  try {
    // Save post and refs via beacon when page is unloading
    window.addEventListener("pagehide", saveBeacon);

    const posts = await listPosts();
    setPosts(posts);

    // Decrypt titles and store them
    const titles = await decryptPostTitles(posts);
    setDecryptedTitles(titles);

    sortPostsByPosition();
    renderPostList();

    if (posts.length > 0) {
      await selectPost(posts[0]);
    } else {
      // Auto-create first post instead of showing empty state
      await handleNewPost();
    }
  } catch (err) {
    console.error("Failed to load posts:", err);
  }
}

export async function loadPostsWithoutSelection(): Promise<void> {
  try {
    const posts = await listPosts();
    setPosts(posts);
    sortPostsByPosition();
    renderPostList();

    showEmptyState("");
  } catch (err) {
    console.error("Failed to load posts:", err);
  }
}

function showEmptyState(message: string): void {
  const container = document.getElementById("editor");
  if (container) {
    container.innerHTML = `<div class="empty-state">${message}</div>`;
  }
}
