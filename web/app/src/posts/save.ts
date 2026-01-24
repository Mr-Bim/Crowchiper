/**
 * Save and encryption logic for posts.
 *
 * Handles debounced encryption, periodic server saves, and beacon saves.
 */

import { updatePost, updatePostBeacon } from "../api/posts.ts";
import { encryptPostData } from "../crypto/post-encryption.ts";
import { callRenderPostList } from "./handlers.ts";
import { parseAttachmentUuids } from "../shared/attachment-utils.ts";
import { clearImageCacheExcept } from "../shared/image-cache.ts";
import {
  clearSaveTimeout,
  clearServerSaveInterval,
  getEditor,
  getIsDirty,
  getLoadedDecryptedContent,
  getLoadedPost,
  getPendingEncryptedData,
  getServerSaveInterval,
  setDecryptedTitle,
  setIsDirty,
  setPendingEncryptedData,
  setSaveTimeout,
  setServerSaveInterval,
} from "./state.ts";

function extractTitle(content: string): string {
  const firstLine = content.split("\n")[0] || "";
  const title = firstLine.replace(/^#*\s*/, "").trim();
  return title || "Untitled";
}

// --- Constants ---

const ENCRYPT_DEBOUNCE_MS = 1000;
const SERVER_SAVE_INTERVAL_MS = 60000;

/**
 * Schedule local encryption after 1 second of inactivity.
 */
export function scheduleEncrypt(): void {
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
export async function encryptCurrentPost(): Promise<void> {
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
    setDecryptedTitle(loadedPost.uuid, title);

    // Mark as dirty (needs server save)
    setIsDirty(true);
    updateSaveButton(true);

    callRenderPostList();

    // Start server save interval if not already running
    startServerSaveInterval();
  } catch (err) {
    console.error("Failed to encrypt:", err);
  }
}

/**
 * Start the periodic server save interval (every 60 seconds).
 */
export function startServerSaveInterval(): void {
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
export function stopServerSaveInterval(): void {
  clearServerSaveInterval();
}

/**
 * Save the pending encrypted data to the server.
 */
export async function saveToServer(): Promise<void> {
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
    updateSaveButton(false);
  } catch (err) {
    console.error("Failed to save to server:", err);
  }
}

/**
 * Save to server immediately when navigating away from a post.
 * Only saves if content has changed.
 */
export async function saveToServerNow(): Promise<void> {
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

  const attachmentUuids = await parseAttachmentUuids(currentContent);

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

// --- Save Button ---

/**
 * Update the save button's visual state.
 * @param dirty - Whether there are unsaved changes
 */
export function updateSaveButton(dirty: boolean): void {
  const btn = document.getElementById("save-btn") as HTMLButtonElement | null;
  if (!btn) return;

  btn.setAttribute("data-dirty", dirty ? "true" : "false");
  btn.textContent = dirty ? "Save" : "Saved";
  btn.disabled = !dirty;
}

/**
 * Handle manual save button click.
 * Encrypts and saves the current post immediately.
 */
export async function handleSave(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  // Clear any pending debounced encryption
  clearSaveTimeout();

  // Encrypt and save immediately
  await encryptCurrentPost();

  const pendingData = getPendingEncryptedData();
  if (!pendingData) return;

  const content = editor.state.doc.toString();
  const attachmentUuids = parseAttachmentUuids(content);

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
    updateSaveButton(false);

    // Clear cache for deleted images
    clearImageCacheExcept(attachmentUuids);
  } catch (err) {
    console.error("Failed to save:", err);
  }
}
