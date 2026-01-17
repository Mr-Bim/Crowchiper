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
  handleNewFolder,
  handleNewPost,
  handleSave,
  loadPosts,
  loadPostsWithoutSelection,
  renderPostList,
} from "./posts/index.ts";
import { setupSpellcheck } from "./spellcheck.ts";
import { createUnlockHandler, showUnlockOverlay } from "./unlock/index.ts";

declare const __TEST_MODE__: boolean;

function setupNewPostDropdown(): void {
  const btn = document.getElementById("new-post-btn");
  const menu = document.getElementById("new-post-menu");
  const newPostOption = document.getElementById("new-post-option");
  const newFolderOption = document.getElementById("new-folder-option");

  if (!btn || !menu) return;

  const showMenu = () => {
    menu.hidden = false;
  };

  const hideMenu = () => {
    menu.hidden = true;
  };

  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    if (menu.hidden) {
      showMenu();
    } else {
      hideMenu();
    }
  });

  newPostOption?.addEventListener("click", () => {
    hideMenu();
    handleNewPost();
  });

  newFolderOption?.addEventListener("click", () => {
    hideMenu();
    handleNewFolder();
  });

  // Close menu when clicking outside
  document.addEventListener("click", (e) => {
    if (!menu.hidden && !menu.contains(e.target as Node)) {
      hideMenu();
    }
  });
}

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

    // Set up spellcheck toggle
    setupSpellcheck();

    // Check encryption settings first
    const settings = await getEncryptionSettings();

    if (settings.encryption_enabled) {
      if (settings.prf_salt) {
        // Use PRF salt for unlock flow
        initEncryption(settings.prf_salt);
      } else {
        throw new Error("Encryption enabled but PRF salt is missing");
      }
    } else {
      disableEncryption();
    }

    // Wire up event handlers
    setupNewPostDropdown();
    document.getElementById("save-btn")?.addEventListener("click", handleSave);
    document
      .getElementById("delete-btn")
      ?.addEventListener("click", handleDeletePost);

    // If encryption is enabled, show unlock overlay
    if (needsUnlock()) {
      // Create unlock handler that loads posts after unlock
      const handleUnlock = createUnlockHandler(loadPosts);
      const unnlockBtn = document.getElementById("unlock-btn");
      unnlockBtn?.addEventListener("click", handleUnlock);

      showUnlockOverlay();
      // Load posts without selecting (they're encrypted)
      await loadPostsWithoutSelection();
      renderPostList();
      if (!__TEST_MODE__) {
        unnlockBtn?.click();
      }
    } else {
      // No encryption or already unlocked - load normally
      await loadPosts();
    }
  } catch (err) {
    console.error("Failed to initialize:", err);
  }
}

document.addEventListener("DOMContentLoaded", init);
