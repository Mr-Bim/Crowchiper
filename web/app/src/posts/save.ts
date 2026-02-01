/**
 * Save and encryption logic for posts.
 *
 * Handles debounced autosave with sync status indicator.
 * Autosave triggers 5 seconds after the last edit.
 */

import { updatePost, updatePostBeacon } from "../api/posts.ts";
import { encryptPostData } from "../crypto/post-encryption.ts";
import { callRenderPostList } from "./handlers.ts";
import { parseAttachmentUuids } from "../shared/attachment-utils.ts";
import { clearImageCacheExcept } from "../shared/image-cache.ts";
import {
  clearSaveTimeout,
  getEditor,
  getLoadedDecryptedContent,
  getLoadedPost,
  getPendingEncryptedData,
  setDecryptedTitle,
  setIsDirty,
  setPendingEncryptedData,
  setSaveTimeout,
  setSyncStatus,
} from "./state/index.ts";

// --- Constants ---

const AUTOSAVE_DEBOUNCE_MS = 5000;
const SYNCED_INDICATOR_MS = 2000;

// --- Helpers ---

function extractTitle(content: string): string {
  const firstLine = content.split("\n")[0] || "";
  const title = firstLine.replace(/^#*\s*/, "").trim();
  return title || "Untitled";
}

/**
 * Build the update payload from pending encrypted data.
 */
function buildUpdatePayload(
  pendingData: NonNullable<ReturnType<typeof getPendingEncryptedData>>,
  attachmentUuids?: string[],
) {
  return {
    title: pendingData.title,
    title_encrypted: pendingData.titleEncrypted,
    title_iv: pendingData.titleIv ?? undefined,
    content: pendingData.content,
    content_encrypted: pendingData.contentEncrypted,
    iv: pendingData.contentIv ?? undefined,
    encryption_version: pendingData.encryptionVersion ?? undefined,
    attachment_uuids: attachmentUuids,
  };
}

// --- Synced indicator timer ---

let syncedTimeout: number | null = null;

function clearSyncedTimeout(): void {
  if (syncedTimeout) {
    clearTimeout(syncedTimeout);
    syncedTimeout = null;
  }
}

// --- Beforeunload Warning ---

function handleBeforeUnload(e: BeforeUnloadEvent): void {
  // Check if there's pending data that hasn't been saved
  if (getPendingEncryptedData()) {
    e.preventDefault();
    e.returnValue = "";
  }
}

export function setupBeforeUnloadWarning(): void {
  window.addEventListener("beforeunload", handleBeforeUnload);
}

// --- Autosave ---

/**
 * Schedule autosave after 5 seconds of inactivity.
 * Updates the title in the sidebar immediately, sets sync status to "pending".
 */
export function scheduleAutosave(): void {
  clearSaveTimeout();
  clearSyncedTimeout();
  setSyncStatus("pending");

  // Update title immediately for responsive UI
  const loadedPost = getLoadedPost();
  const editor = getEditor();
  if (loadedPost && editor) {
    const content = editor.state.doc.toString();
    const title = extractTitle(content);
    setDecryptedTitle(loadedPost.uuid, title);
    callRenderPostList();
  }

  setSaveTimeout(
    window.setTimeout(() => {
      autosave();
    }, AUTOSAVE_DEBOUNCE_MS),
  );
}

/**
 * Autosave: encrypt and save to server.
 */
async function autosave(): Promise<void> {
  await encryptCurrentPost();
  await savePost({ includeAttachments: true, clearCache: true });
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

    setPendingEncryptedData({
      title: encrypted.title,
      titleEncrypted: encrypted.titleEncrypted,
      titleIv: encrypted.titleIv ?? null,
      content: encrypted.content,
      contentEncrypted: encrypted.contentEncrypted,
      contentIv: encrypted.contentIv ?? null,
      encryptionVersion: encrypted.encryptionVersion ?? null,
    });

    setDecryptedTitle(loadedPost.uuid, title);
    setIsDirty(true);
    callRenderPostList();
  } catch (err) {
    console.error("Failed to encrypt:", err);
    setSyncStatus("error");
  }
}

// --- Core Save Function ---

interface SaveOptions {
  /** Include attachment UUIDs in the save (parses content for refs) */
  includeAttachments?: boolean;
  /** Clear image cache for deleted images */
  clearCache?: boolean;
}

/**
 * Core save function - saves pending encrypted data to server.
 * Used by all save paths except beacon (which uses sendBeacon).
 */
async function savePost(options: SaveOptions = {}): Promise<void> {
  const loadedPost = getLoadedPost();
  const pendingData = getPendingEncryptedData();

  if (!loadedPost || !pendingData) return;

  const { includeAttachments, clearCache } = options;

  setSyncStatus("syncing");

  try {
    let attachmentUuids: string[] | undefined;
    if (includeAttachments) {
      const editor = getEditor();
      if (editor) {
        const content = editor.state.doc.toString();
        attachmentUuids = await parseAttachmentUuids(content);
      }
    }

    await updatePost(
      loadedPost.uuid,
      buildUpdatePayload(pendingData, attachmentUuids),
    );

    // Clear pending data after successful save
    setPendingEncryptedData(null);
    setIsDirty(false);

    // Show synced indicator briefly, then return to idle
    setSyncStatus("synced");
    clearSyncedTimeout();
    syncedTimeout = window.setTimeout(() => {
      setSyncStatus("idle");
    }, SYNCED_INDICATOR_MS);

    if (clearCache && attachmentUuids) {
      clearImageCacheExcept(attachmentUuids);
    }
  } catch (err) {
    console.error("Failed to save to server:", err);
    setSyncStatus("error");
  }
}

// --- Public Save APIs ---

/**
 * Save to server immediately when navigating away from a post.
 * Includes attachments and clears cache.
 */
export async function saveToServerNow(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  const currentContent = editor.state.doc.toString();
  const originalContent = getLoadedDecryptedContent();

  // Only save if content has actually changed
  if (currentContent === originalContent) return;

  clearSaveTimeout();
  await encryptCurrentPost();
  await savePost({ includeAttachments: true, clearCache: true });
}

/**
 * Force save for testing purposes.
 * Immediately encrypts and saves the current post.
 */
export async function forceSave(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  clearSaveTimeout();
  clearSyncedTimeout();
  await encryptCurrentPost();
  await savePost({ includeAttachments: true, clearCache: true });
}

/**
 * Save post via beacon when page is unloading.
 * Uses sendBeacon for reliability - cannot be async.
 */
export function saveBeacon(): void {
  const loadedPost = getLoadedPost();
  const editor = getEditor();
  const pendingData = getPendingEncryptedData();

  if (!loadedPost || !editor || !pendingData) return;

  const content = editor.state.doc.toString();
  const attachmentUuids = parseAttachmentUuids(content);

  updatePostBeacon(
    loadedPost.uuid,
    buildUpdatePayload(pendingData, attachmentUuids),
  );
}
