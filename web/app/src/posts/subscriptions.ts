/**
 * Reactive subscriptions for automatic UI updates.
 *
 * Sets up signal subscriptions to update the UI when state changes.
 * Call initSubscriptions() once during app initialization.
 */

import { getOptionalElement } from "../../../shared/dom.ts";
import { syncStatusSignal, type SyncStatus } from "./state/index.ts";

let initialized = false;

/**
 * Update the sync indicator based on sync status.
 */
function updateSyncIndicator(status: SyncStatus): void {
  const indicator = getOptionalElement("sync-indicator");
  if (!indicator) return;

  indicator.setAttribute("data-status", status);
}

/**
 * Initialize reactive subscriptions.
 * Should be called once during app initialization, after DOM is ready.
 */
export function initSubscriptions(): void {
  if (initialized) return;
  initialized = true;

  // Auto-update sync indicator when sync status changes
  syncStatusSignal.subscribe(updateSyncIndicator);

  // Set initial state
  updateSyncIndicator(syncStatusSignal.get());
}
