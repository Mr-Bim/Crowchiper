/**
 * Toast notification for displaying errors to the user.
 */

import { getOptionalElement } from "../../shared/dom.ts";

let hideTimeout: ReturnType<typeof setTimeout> | null = null;
let currentCloseHandler: (() => void) | null = null;

/**
 * Hide the toast and clear any pending timeout.
 */
function hideToast(toast: HTMLElement): void {
  if (hideTimeout) {
    clearTimeout(hideTimeout);
    hideTimeout = null;
  }
  toast.hidden = true;
}

/**
 * Show an error toast notification.
 * The toast auto-hides after 5 seconds, or can be dismissed manually.
 */
export function showError(message: string): void {
  const toast = getOptionalElement("error-toast");
  const messageEl = getOptionalElement("error-toast-message");
  const closeBtn = getOptionalElement("error-toast-close", HTMLButtonElement);

  if (!toast || !messageEl || !closeBtn) {
    console.error("Error toast elements not found");
    return;
  }

  // Clear any existing timeout
  if (hideTimeout) {
    clearTimeout(hideTimeout);
    hideTimeout = null;
  }

  // Remove old handler if it exists
  if (currentCloseHandler) {
    closeBtn.removeEventListener("click", currentCloseHandler);
  }

  // Create and store new handler
  currentCloseHandler = () => hideToast(toast);
  closeBtn.addEventListener("click", currentCloseHandler);

  messageEl.textContent = message;
  toast.hidden = false;

  // Auto-hide after 5 seconds
  hideTimeout = setTimeout(() => {
    toast.hidden = true;
  }, 5000);
}
