/**
 * Spellcheck toggle functionality.
 *
 * Manages OS spellcheck state for the editor, persisted to localStorage.
 */

const STORAGE_KEY = "spellcheck-enabled";

let spellcheckEnabled = false;

/**
 * Get the current spellcheck state.
 */
export function isSpellcheckEnabled(): boolean {
  return spellcheckEnabled;
}

/**
 * Load spellcheck preference from localStorage.
 */
export function loadSpellcheckPreference(): void {
  const stored = localStorage.getItem(STORAGE_KEY);
  spellcheckEnabled = stored === "true";
}

/**
 * Save spellcheck preference to localStorage.
 */
function saveSpellcheckPreference(): void {
  localStorage.setItem(STORAGE_KEY, spellcheckEnabled ? "true" : "false");
}

/**
 * Update the spellcheck button UI.
 */
function updateSpellcheckButton(): void {
  const btn = document.getElementById("spellcheck-btn");
  if (btn) {
    btn.setAttribute("data-enabled", spellcheckEnabled ? "true" : "false");
    btn.title = spellcheckEnabled ? "Disable spellcheck" : "Enable spellcheck";
  }
}

/**
 * Apply spellcheck attribute to the editor content.
 */
export function applySpellcheckToEditor(): void {
  const editorEl = document.getElementById("editor");
  if (!editorEl) return;

  const contentEl = editorEl.querySelector(".cm-content");
  if (contentEl instanceof HTMLElement) {
    contentEl.setAttribute("spellcheck", spellcheckEnabled ? "true" : "false");
  }
}

/**
 * Toggle spellcheck on/off.
 */
export function toggleSpellcheck(): void {
  spellcheckEnabled = !spellcheckEnabled;
  saveSpellcheckPreference();
  updateSpellcheckButton();
  applySpellcheckToEditor();
}

/**
 * Initialize spellcheck functionality.
 */
export function setupSpellcheck(): void {
  loadSpellcheckPreference();
  updateSpellcheckButton();

  const btn = document.getElementById("spellcheck-btn");
  btn?.addEventListener("click", toggleSpellcheck);
}
