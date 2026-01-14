/**
 * Toast notification for displaying errors to the user.
 */

let hideTimeout: ReturnType<typeof setTimeout> | null = null;

/**
 * Show an error toast notification.
 * The toast auto-hides after 5 seconds, or can be dismissed manually.
 */
export function showError(message: string): void {
  const toast = document.getElementById("error-toast") as HTMLElement;
  const messageEl = document.getElementById(
    "error-toast-message",
  ) as HTMLElement;
  const closeBtn = document.getElementById(
    "error-toast-close",
  ) as HTMLButtonElement;

  if (!toast || !messageEl || !closeBtn) {
    console.error("Error toast elements not found");
    return;
  }

  // Clear any existing timeout
  if (hideTimeout) {
    clearTimeout(hideTimeout);
    hideTimeout = null;
  }

  messageEl.textContent = message;
  toast.hidden = false;

  // Auto-hide after 5 seconds
  hideTimeout = setTimeout(() => {
    toast.hidden = true;
  }, 5000);

  // Set up close button handler (remove old one first to avoid duplicates)
  const newCloseBtn = closeBtn.cloneNode(true) as HTMLButtonElement;
  closeBtn.parentNode?.replaceChild(newCloseBtn, closeBtn);
  newCloseBtn.addEventListener("click", () => {
    if (hideTimeout) {
      clearTimeout(hideTimeout);
      hideTimeout = null;
    }
    toast.hidden = true;
  });
}
