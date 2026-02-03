/**
 * Post list rendering.
 *
 * Handles rendering the post tree and expand/collapse functionality.
 */

import { listPostChildren, type PostNode } from "../api/posts.ts";
import { decryptPostTitles } from "../crypto/post-encryption.ts";
import { getOptionalElement } from "../../../shared/dom.ts";
import { chevron } from "../../../shared/icons/chevron-down.ts";
import {
  getReorderHandler,
  getReparentHandler,
  getSelectPostHandler,
  isHandlersRegistered,
} from "./handlers.ts";
import {
  flattenPosts,
  getDecryptedTitles,
  getLoadedPost,
  getPosts,
  isExpanded,
  setDecryptedTitles,
  setPostChildren,
  toggleExpanded,
} from "./state/index.ts";

declare const __TEST_MODE__: boolean;

/**
 * Render a single post node and its children recursively.
 */
export function renderPostNode(
  post: PostNode,
  depth: number,
  index: number,
): HTMLElement {
  const loadedPost = getLoadedPost();
  const decryptedTitles = getDecryptedTitles();
  const expanded = isExpanded(post.uuid);

  // Wrapper div for drag and drop
  const wrapper = document.createElement("div");
  wrapper.className = "post-wrapper";
  if (__TEST_MODE__) {
    wrapper.setAttribute("data-testid", "test-post-wrapper");
  }
  wrapper.setAttribute("data-post-uuid", post.uuid);
  wrapper.setAttribute("data-index", String(index));
  wrapper.setAttribute("data-depth", String(Math.min(3, depth)));
  wrapper.setAttribute("data-parent-id", post.parent_id ?? "");

  // Container for the post item (expand button + title button)
  const itemContainer = document.createElement("div");
  itemContainer.className = "post-item-container";
  if (__TEST_MODE__) {
    itemContainer.setAttribute("data-testid", "test-post-item-container");
  }
  itemContainer.setAttribute("data-depth", String(Math.min(3, depth)));

  // Expand/collapse button (only if has children)
  if (post.has_children) {
    const expandBtn = document.createElement("button");
    expandBtn.className = "ghost post-expand-btn";
    if (__TEST_MODE__) {
      expandBtn.setAttribute("data-testid", "test-post-expand-btn");
    }
    expandBtn.setAttribute("data-post-expanded", String(expanded));
    expandBtn.innerHTML = `<span class="chevron-expand" data-testid="test-chevron">${chevron}</span>`;
    expandBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      handleToggleExpand(post);
    });
    itemContainer.appendChild(expandBtn);
  } else {
    // Spacer for alignment
    const spacer = document.createElement("span");
    spacer.className = "post-expand-spacer";
    itemContainer.appendChild(spacer);
  }

  // Post icon
  const icon = document.createElement("span");
  icon.className = "post-icon";
  icon.textContent = "\uD83D\uDCC4"; // 📄
  itemContainer.appendChild(icon);

  // Button for selection
  const item = document.createElement("button");
  item.className = "cl-post-item";
  item.classList = item.classList + " ghost";
  if (__TEST_MODE__) {
    item.setAttribute("data-testid", "test-post-item");
  }
  if (loadedPost?.uuid === post.uuid) {
    item.classList.add("active");
    item.setAttribute("aria-current", "page");
  }

  // Use decrypted title from map, fallback to post.title, then "Untitled"
  const title = decryptedTitles.get(post.uuid) ?? post.title ?? "Untitled";
  item.textContent = title;
  item.title = title; // Show full title on hover

  // Click to select for editing
  item.addEventListener("click", () => {
    getSelectPostHandler()(post);
  });

  itemContainer.appendChild(item);
  wrapper.appendChild(itemContainer);

  return wrapper;
}

/**
 * Render the entire post tree.
 */
export function renderPostList(): void {
  const list = getOptionalElement("post-list");
  if (!list) return;

  const posts = getPosts();
  list.innerHTML = "";

  // Render tree recursively
  let globalIndex = 0;
  const listElement = list; // Capture for closure
  function renderLevel(nodes: PostNode[], depth: number): void {
    for (const post of nodes) {
      const wrapper = renderPostNode(post, depth, globalIndex++);
      listElement.appendChild(wrapper);

      // Render children if expanded
      if (post.has_children && isExpanded(post.uuid) && post.children) {
        renderLevel(post.children, depth + 1);
      }
    }
  }

  renderLevel(posts, 0);

  if (isHandlersRegistered()) {
    // Initialize drag and drop on the list (lazy-loaded)
    import("./drag-and-drop.ts").then(({ initDragAndDrop }) => {
      initDragAndDrop(list, getReorderHandler(), getReparentHandler());
    });
  }
}

/**
 * Re-initialize drag and drop on the post list.
 */
export function reinitDragAndDrop(list: HTMLElement): void {
  if (isHandlersRegistered()) {
    import("./drag-and-drop.ts").then(({ initDragAndDrop }) => {
      initDragAndDrop(list, getReorderHandler(), getReparentHandler());
    });
  }
}

/**
 * Handle expand/collapse toggle for a post.
 */
export async function handleToggleExpand(post: PostNode): Promise<void> {
  // If children not loaded yet, fetch them first
  if (post.has_children && post.children === null) {
    try {
      const children = await listPostChildren(post.uuid);
      setPostChildren(post.uuid, children);

      // Decrypt titles for the new children
      const allPosts = flattenPosts();
      const titles = await decryptPostTitles(allPosts);
      setDecryptedTitles(titles);
    } catch (err) {
      console.error("Failed to load children:", err);
      return;
    }
  }

  // Toggle expanded state - the signal subscription handles the chevron animation
  toggleExpanded(post.uuid);
}
