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
  initSubscriptions,
  loadPosts,
  renderPostList,
} from "./posts/index.ts";
import { setupSpellcheck } from "./spellcheck.ts";
import { createUnlockHandler, showUnlockOverlay } from "./unlock/index.ts";
import { getOptionalElement } from "../../shared/dom.ts";

declare const __TEST_MODE__: boolean;
declare const API_PATH: string;
declare const LOGIN_PATH: string;

function setupNewPostButton(): void {
  const btn = getOptionalElement("new-post-btn");
  if (!btn) return;

  btn.addEventListener("click", () => {
    handleNewPost();
  });
}

/**
 * Set up global keyboard shortcuts.
 * - Ctrl/Cmd+S: Save current post
 * - Ctrl/Cmd+N: Create new post
 */
function setupKeyboardShortcuts(): void {
  document.addEventListener("keydown", (e) => {
    const isMod = e.ctrlKey || e.metaKey;
    if (!isMod) return;

    if (e.key === "s") {
      e.preventDefault();
      handleSave();
    } else if (e.key === "n") {
      e.preventDefault();
      handleNewPost();
    }
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

/**
 * Set up the trigger for the settings panel.
 * The actual panel module is lazy-loaded when first opened.
 */
function setupSettingsPanelTrigger(): void {
  const manageSessionsBtn = getOptionalElement("manage-sessions-btn");
  const settingsMenu = getOptionalElement("settings-menu");
  const settingsBtn = getOptionalElement("settings-btn");

  if (manageSessionsBtn) {
    manageSessionsBtn.addEventListener("click", async () => {
      // Close the settings dropdown menu
      if (settingsMenu) {
        settingsMenu.hidden = true;
      }
      if (settingsBtn) {
        settingsBtn.setAttribute("aria-expanded", "false");
      }

      // Lazy-load and open the settings panel
      const { openSettingsPanel, setupSettingsPanel } =
        await import("./settings-panel.ts");
      setupSettingsPanel();
      openSettingsPanel();
    });
  }
}

function setupSidebarToggle(): void {
  const sidebar = getOptionalElement("sidebar");
  const toggleBtn = getOptionalElement("sidebar-toggle");
  const editor = getOptionalElement("editor");

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
    // Initialize reactive subscriptions (must be before any state changes)
    initSubscriptions();

    // Set up bfcache handler to verify auth on page restore
    setupBfcacheHandler();

    // Set up sidebar toggle for mobile
    setupSidebarToggle();

    // Set up spellcheck toggle
    setupSpellcheck();

    // Set up settings panel (lazy-loaded on first open)
    setupSettingsPanelTrigger();

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
    setupKeyboardShortcuts();
    getOptionalElement("save-btn")?.addEventListener("click", handleSave);
    getOptionalElement("delete-btn")?.addEventListener(
      "click",
      handleDeletePost,
    );

    // If encryption is enabled, show unlock overlay
    if (needsUnlock()) {
      // Create unlock handler that loads posts after unlock
      const handleUnlock = createUnlockHandler(loadPosts);
      const unnlockBtn = getOptionalElement("unlock-btn");

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
