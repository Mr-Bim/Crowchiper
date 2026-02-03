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
  tryRestoreDevKey,
} from "./crypto/keystore.ts";
import {
  handleDeletePost,
  handleNewPost,
  forceSave,
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

/**
 * Set up the settings dropdown menu toggle behavior.
 */
function setupSettingsMenu(): void {
  const settingsBtn = getOptionalElement("settings-btn");
  const settingsMenu = getOptionalElement("settings-menu");

  if (!settingsBtn || !settingsMenu) return;

  settingsBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    const isOpen = !settingsMenu.hidden;
    settingsMenu.hidden = isOpen;
    settingsBtn.setAttribute("aria-expanded", String(!isOpen));
  });

  // Close menu when clicking outside
  document.addEventListener("click", (e) => {
    if (
      !settingsMenu.hidden &&
      !settingsMenu.contains(e.target as Node) &&
      !settingsBtn.contains(e.target as Node)
    ) {
      settingsMenu.hidden = true;
      settingsBtn.setAttribute("aria-expanded", "false");
    }
  });

  // Close menu on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !settingsMenu.hidden) {
      settingsMenu.hidden = true;
      settingsBtn.setAttribute("aria-expanded", "false");
    }
  });
}

/**
 * Set up the logout button.
 */
function setupLogoutButton(): void {
  const logoutBtn = getOptionalElement("logout-btn");
  if (!logoutBtn) return;

  logoutBtn.addEventListener("click", async () => {
    try {
      await fetch(`${API_PATH}/tokens/logout`, {
        method: "POST",
        credentials: "include",
      });
      window.location.href = LOGIN_PATH;
    } catch {
      // Redirect even on error
      window.location.href = LOGIN_PATH;
    }
  });
}

interface ConfigWithVersion {
  version: string;
  git_hash: string;
}

// Cache for version info from /config
let cachedVersionInfo: ConfigWithVersion | null = null;

async function fetchVersionInfo(): Promise<ConfigWithVersion | null> {
  if (cachedVersionInfo) return cachedVersionInfo;
  try {
    const response = await fetch(`${API_PATH}/config`, {
      credentials: "include",
    });
    if (response.ok) {
      cachedVersionInfo = await response.json();
      return cachedVersionInfo;
    }
  } catch {
    // Ignore errors
  }
  return null;
}

/**
 * Set up the version modal and button.
 */
function setupVersionModal(): void {
  const versionBtn = getOptionalElement("version-btn");
  const versionModal = getOptionalElement("version-modal");
  const versionModalClose = getOptionalElement("version-modal-close");
  const versionLabel = getOptionalElement("version-label");
  const versionValue = getOptionalElement("version-value");
  const buildValue = getOptionalElement("build-value");
  const settingsMenu = getOptionalElement("settings-menu");
  const settingsBtn = getOptionalElement("settings-btn");

  const updateVersionDisplay = (info: ConfigWithVersion) => {
    if (versionLabel) versionLabel.textContent = `v${info.version}`;
    if (versionValue) versionValue.textContent = info.version;
    // Show only first 7 characters of the hash (short hash)
    if (buildValue) buildValue.textContent = info.git_hash.slice(0, 7);
  };

  const openVersionModal = () => {
    if (versionModal) versionModal.hidden = false;
  };

  const closeVersionModal = () => {
    if (versionModal) versionModal.hidden = true;
  };

  if (versionBtn) {
    versionBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      // Close settings menu
      if (settingsMenu) {
        settingsMenu.hidden = true;
        settingsBtn?.setAttribute("aria-expanded", "false");
      }
      openVersionModal();
    });
  }

  if (versionModalClose) {
    versionModalClose.addEventListener("click", closeVersionModal);
  }

  // Close modal when clicking overlay
  if (versionModal) {
    versionModal.addEventListener("click", (e) => {
      if (e.target === versionModal) {
        closeVersionModal();
      }
    });
  }

  // Close modal on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && versionModal && !versionModal.hidden) {
      closeVersionModal();
    }
  });

  // Fetch version info on load to update button label
  fetchVersionInfo().then((info) => {
    if (info) {
      updateVersionDisplay(info);
    }
  });
}

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
      forceSave();
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

    // Set up settings menu dropdown toggle
    setupSettingsMenu();

    // Set up logout button
    setupLogoutButton();

    // Set up version modal
    setupVersionModal();

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
    // Add force save button only in test mode
    if (__TEST_MODE__) {
      const syncIndicator = getOptionalElement("sync-indicator");
      if (syncIndicator) {
        const forceSaveBtn = document.createElement("button");
        forceSaveBtn.id = "force-save-btn";
        forceSaveBtn.className = "info cl-save-btn";
        forceSaveBtn.textContent = "Save";
        forceSaveBtn.setAttribute("data-testid", "test-force-save-btn");
        syncIndicator.insertAdjacentElement("afterend", forceSaveBtn);
        forceSaveBtn.addEventListener("click", forceSave);
      }
    }
    getOptionalElement("delete-btn")?.addEventListener(
      "click",
      handleDeletePost,
    );

    // Try to restore dev key from sessionStorage (dev mode only, no-op in prod)
    const devKeyRestored = await tryRestoreDevKey();

    // If encryption is enabled, show unlock overlay (unless dev key was restored)
    if (needsUnlock() && !devKeyRestored) {
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
