/**
 * Save and encryption logic for posts.
 *
 * Handles debounced encryption, periodic server saves, and beacon saves.
 * Note: Save button UI updates are handled reactively via isDirtySignal subscription.
 */

import { updatePost, updatePostBeacon } from "../api/posts.ts";
import { encryptPostData } from "../crypto/post-encryption.ts";
import { callRenderPostList } from "./handlers.ts";
import { parseAttachmentUuids } from "../shared/attachment-utils.ts";
import { clearImageCacheExcept } from "../shared/image-cache.ts";
import { showSuccess } from "../toast.ts";
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
} from "./state/index.ts";

// --- Constants ---

const ENCRYPT_DEBOUNCE_MS = 1000;
const SERVER_SAVE_INTERVAL_MS = 60000;

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

// --- Beforeunload Warning ---

function handleBeforeUnload(e: BeforeUnloadEvent): void {
  if (getIsDirty()) {
    e.preventDefault();
    e.returnValue = "";
  }
}

export function setupBeforeUnloadWarning(): void {
  window.addEventListener("beforeunload", handleBeforeUnload);
}

// --- Encryption ---

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
    startServerSaveInterval();
  } catch (err) {
    console.error("Failed to encrypt:", err);
  }
}

// --- Server Save Interval ---

export function startServerSaveInterval(): void {
  if (getServerSaveInterval()) return;

  setServerSaveInterval(
    window.setInterval(() => {
      savePost();
    }, SERVER_SAVE_INTERVAL_MS),
  );
}

export function stopServerSaveInterval(): void {
  clearServerSaveInterval();
}

// --- Core Save Function ---

interface SaveOptions {
  /** Include attachment UUIDs in the save (parses content for refs) */
  includeAttachments?: boolean;
  /** Show success toast after save */
  showToast?: boolean;
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

  if (!loadedPost || !pendingData || !getIsDirty()) return;

  const {
    includeAttachments,
    showToast: shouldShowToast,
    clearCache,
  } = options;

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
    setIsDirty(false);

    if (shouldShowToast) {
      showSuccess("Saved");
    }

    if (clearCache && attachmentUuids) {
      clearImageCacheExcept(attachmentUuids);
    }
  } catch (err) {
    console.error("Failed to save to server:", err);
  }
}

// --- Public Save APIs ---

/**
 * Save to server (used by periodic interval).
 * Minimal save - no attachments, no toast, no cache clear.
 */
export async function saveToServer(): Promise<void> {
  await savePost();
}

/**
 * Save to server immediately when navigating away from a post.
 * Includes attachments and clears cache, but no toast.
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
 * Handle manual save button click.
 * Full save with attachments, toast, and cache clear.
 */
export async function handleSave(): Promise<void> {
  const loadedPost = getLoadedPost();
  const editor = getEditor();

  if (!loadedPost || !editor) return;

  clearSaveTimeout();
  await encryptCurrentPost();
  await savePost({
    includeAttachments: true,
    showToast: true,
    clearCache: true,
  });
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
