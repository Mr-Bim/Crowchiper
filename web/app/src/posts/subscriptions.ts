/**
 * Reactive subscriptions for automatic UI updates.
 *
 * Sets up signal subscriptions to update the UI when state changes.
 * Call initSubscriptions() once during app initialization.
 */

import { getOptionalElement } from "../../../shared/dom.ts";
import { isDirtySignal } from "./state/index.ts";

let initialized = false;

/**
 * Update the save button based on dirty state.
 */
function updateSaveButton(dirty: boolean): void {
  const btn = getOptionalElement("save-btn", HTMLButtonElement);
  if (!btn) return;

  btn.setAttribute("data-dirty", dirty ? "true" : "false");
  btn.textContent = dirty ? "Save" : "Saved";
  btn.disabled = !dirty;
}

/**
 * Initialize reactive subscriptions.
 * Should be called once during app initialization, after DOM is ready.
 */
export function initSubscriptions(): void {
  if (initialized) return;
  initialized = true;

  // Auto-update save button when dirty state changes
  isDirtySignal.subscribe(updateSaveButton);

  // Set initial state
  updateSaveButton(isDirtySignal.get());
}
