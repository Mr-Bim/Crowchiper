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
  handleSave,
  loadPosts,
  renderPostList,
} from "./posts/index.ts";
import { setupSpellcheck } from "./spellcheck.ts";
import { createUnlockHandler, showUnlockOverlay } from "./unlock/index.ts";

declare const __TEST_MODE__: boolean;
declare const API_PATH: string;
declare const LOGIN_PATH: string;

function setupNewPostButton(): void {
  const btn = document.getElementById("new-post-btn");
  if (!btn) return;

  btn.addEventListener("click", () => {
    handleNewPost();
  });
}

/**
 * Check if the user is still authenticated.
 * Used when the page is restored from bfcache after logout.
 * Server auto-refreshes access token if refresh token is valid.
 */
async function verifyAuth(): Promise<boolean> {
  try {
    const response = await fetch(`${API_PATH}/tokens/verify`, {
      credentials: "include",
    });
    return response.ok;
  } catch {
    return false;
  }
}

/**
 * Set up handler for bfcache restoration.
 * When the page is restored from bfcache (e.g., after pressing back),
 * verify that the user is still authenticated.
 */
function setupBfcacheHandler(): void {
  window.addEventListener("pageshow", async (event) => {
    if (event.persisted) {
      const isAuthenticated = await verifyAuth();
      if (!isAuthenticated) {
        window.location.href = LOGIN_PATH;
      }
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
    // Set up bfcache handler to verify auth on page restore
    setupBfcacheHandler();

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
    setupNewPostButton();
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
      // await loadPostsWithoutSelection();
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
