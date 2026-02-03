/**
 * Reactive subscriptions for automatic UI updates.
 *
 * Sets up signal subscriptions to update the UI when state changes.
 * Call initSubscriptions() once during app initialization.
 */

import type { PostNode } from "../api/posts.ts";
import { getOptionalElement } from "../../../shared/dom.ts";
import {
  expandedChangedSignal,
  findPost,
  isExpanded,
  syncStatusSignal,
  type SyncStatus,
} from "./state/index.ts";
import { renderPostNode, reinitDragAndDrop } from "./render.ts";

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
 * Handle expand/collapse: update chevron in-place and surgically add/remove children.
 */
function handleExpandedChange(
  change: { uuid: string; expanded: boolean } | null,
): void {
  if (!change) return;

  const list = getOptionalElement("post-list");
  if (!list) return;

  const { uuid, expanded } = change;

  const wrapper = list.querySelector<HTMLElement>(`[data-post-uuid="${uuid}"]`);
  if (!wrapper) return;

  const expandBtn = wrapper.querySelector<HTMLElement>("[data-post-expanded]");
  if (expandBtn) {
    expandBtn.setAttribute("data-post-expanded", String(expanded));
  }

  if (expanded) {
    // Expanding: insert children after the wrapper
    const post = findPost(uuid);
    if (post?.children) {
      const depth = parseInt(wrapper.getAttribute("data-depth") || "0", 10) + 1;
      let insertAfter: Element = wrapper;
      let index = parseInt(wrapper.getAttribute("data-index") || "0", 10) + 1;

      for (const child of post.children) {
        const childEl = renderPostNode(child, depth, index++);
        insertAfter.after(childEl);
        insertAfter = childEl;

        // Recursively insert if child is also expanded
        if (child.has_children && isExpanded(child.uuid) && child.children) {
          const result = insertExpandedDescendants(
            child,
            insertAfter,
            depth + 1,
            index,
          );
          insertAfter = result.lastElement;
          index = result.nextIndex;
        }
      }
    }
  } else {
    // Collapsing: remove all descendant elements
    removeDescendantElements(list, uuid);
  }

  // Re-initialize drag and drop for new elements
  reinitDragAndDrop(list);
}

/**
 * Recursively insert expanded descendants.
 */
function insertExpandedDescendants(
  post: PostNode,
  insertAfter: Element,
  depth: number,
  startIndex: number,
): { lastElement: Element; nextIndex: number } {
  let current = insertAfter;
  let index = startIndex;

  if (!post.children) return { lastElement: current, nextIndex: index };

  for (const child of post.children) {
    const childEl = renderPostNode(child, depth, index++);
    current.after(childEl);
    current = childEl;

    if (child.has_children && isExpanded(child.uuid) && child.children) {
      const result = insertExpandedDescendants(
        child,
        current,
        depth + 1,
        index,
      );
      current = result.lastElement;
      index = result.nextIndex;
    }
  }

  return { lastElement: current, nextIndex: index };
}

/**
 * Remove all descendant elements of a post from the DOM.
 */
function removeDescendantElements(list: HTMLElement, parentUuid: string): void {
  const post = findPost(parentUuid);
  if (!post?.children) return;

  const uuidsToRemove = collectDescendantUuids(post);
  for (const uuid of uuidsToRemove) {
    const el = list.querySelector(`[data-post-uuid="${uuid}"]`);
    el?.remove();
  }
}

/**
 * Collect all descendant UUIDs (only those currently visible/expanded).
 */
function collectDescendantUuids(post: PostNode): string[] {
  const result: string[] = [];

  function collect(nodes: PostNode[] | null | undefined): void {
    if (!nodes) return;
    for (const node of nodes) {
      result.push(node.uuid);
      // Only collect children if they were expanded (visible in DOM)
      if (node.has_children && isExpanded(node.uuid)) {
        collect(node.children);
      }
    }
  }

  collect(post.children);
  return result;
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

  // Auto-update chevron and surgically add/remove children
  expandedChangedSignal.subscribe(handleExpandedChange);

  // Set initial state
  updateSyncIndicator(syncStatusSignal.get());
}
