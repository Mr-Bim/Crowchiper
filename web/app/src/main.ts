/**
 * Main entry point for the app.
 *
 * Initializes encryption settings, wires up event handlers,
 * and coordinates the unlock flow.
 */

import { getEncryptionSettings } from "./api/encryption-settings.ts";
import {
  disableEncryption,
  initEncryption,
  needsUnlock,
} from "./crypto/keystore.ts";
import {
  handleDeletePost,
  handleNewPost,
  loadPosts,
  loadPostsWithoutSelection,
  renderPostList,
} from "./posts/ui.ts";
import { createUnlockHandler, showUnlockOverlay } from "./unlock/index.ts";

function setupSidebarToggle(): void {
  const sidebar = document.getElementById("sidebar");
  const toggleBtn = document.getElementById("sidebar-toggle");
  const editor = document.getElementById("editor");

  if (!sidebar || !toggleBtn) return;

  const collapseSidebar = () => {
    sidebar.setAttribute("data-collapsed", "");
    toggleBtn.setAttribute("aria-expanded", "false");
  };

  const expandSidebar = () => {
    sidebar.removeAttribute("data-collapsed");
    toggleBtn.setAttribute("aria-expanded", "true");
  };

  toggleBtn.addEventListener("click", () => {
    const isCollapsed = sidebar.hasAttribute("data-collapsed");
    if (isCollapsed) {
      expandSidebar();
    } else {
      collapseSidebar();
    }
  });

  // Collapse sidebar when clicking on the editor
  if (editor) {
    editor.addEventListener("click", () => {
      const isCollapsed = sidebar.hasAttribute("data-collapsed");
      if (!isCollapsed) {
        collapseSidebar();
      }
    });
  }
}

async function init(): Promise<void> {
  try {
    // Set up sidebar toggle for mobile
    setupSidebarToggle();

    // Check encryption settings first
    const settings = await getEncryptionSettings();

    if (settings.encryption_enabled) {
      if (!settings.prf_salt) {
        throw new Error("Encryption enabled but PRF salt is missing");
      }
      initEncryption(settings.prf_salt);
    } else {
      disableEncryption();
    }

    // Wire up event handlers
    document
      .getElementById("new-post-btn")
      ?.addEventListener("click", handleNewPost);
    document
      .getElementById("delete-btn")
      ?.addEventListener("click", handleDeletePost);

    // If encryption is enabled, show unlock overlay
    if (needsUnlock()) {
      // Create unlock handler that loads posts after unlock
      const handleUnlock = createUnlockHandler(loadPosts);
      document
        .getElementById("unlock-btn")
        ?.addEventListener("click", handleUnlock);

      showUnlockOverlay();

      // Load posts without selecting (they're encrypted)
      await loadPostsWithoutSelection();
      renderPostList();
    } else {
      // No encryption or already unlocked - load normally
      await loadPosts();
    }
  } catch (err) {
    console.error("Failed to initialize:", err);
  }
}

document.addEventListener("DOMContentLoaded", init);
