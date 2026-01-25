/**
 * Toast notifications for displaying messages to the user.
 */

import { getOptionalElement } from "../../shared/dom.ts";

let errorHideTimeout: ReturnType<typeof setTimeout> | null = null;
let successHideTimeout: ReturnType<typeof setTimeout> | null = null;
let currentCloseHandler: (() => void) | null = null;

/**
 * Hide the error toast and clear any pending timeout.
 */
function hideErrorToast(toast: HTMLElement): void {
  if (errorHideTimeout) {
    clearTimeout(errorHideTimeout);
    errorHideTimeout = null;
  }
  toast.hidden = true;
}

/**
 * Show an error toast notification.
 * The toast auto-hides after 10 seconds, or can be dismissed manually.
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
  if (errorHideTimeout) {
    clearTimeout(errorHideTimeout);
    errorHideTimeout = null;
  }

  // Remove old handler if it exists
  if (currentCloseHandler) {
    closeBtn.removeEventListener("click", currentCloseHandler);
  }

  // Create and store new handler
  currentCloseHandler = () => hideErrorToast(toast);
  closeBtn.addEventListener("click", currentCloseHandler);

  messageEl.textContent = message;
  toast.hidden = false;

  // Auto-hide after 10 seconds (longer for important errors)
  errorHideTimeout = setTimeout(() => {
    toast.hidden = true;
  }, 10000);
}

/**
 * Show a success toast notification.
 * The toast auto-hides after 2 seconds.
 */
export function showSuccess(message: string): void {
  const toast = getOptionalElement("success-toast");
  const messageEl = getOptionalElement("success-toast-message");

  if (!toast || !messageEl) {
    return;
  }

  // Clear any existing timeout
  if (successHideTimeout) {
    clearTimeout(successHideTimeout);
    successHideTimeout = null;
  }

  messageEl.textContent = message;
  toast.hidden = false;

  // Auto-hide after 2 seconds
  successHideTimeout = setTimeout(() => {
    toast.hidden = true;
  }, 2000);
}
