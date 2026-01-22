/**
 * Handler registry for post operations.
 *
 * This module breaks circular dependencies by providing a central place
 * to register and retrieve handlers. Modules register their handlers here
 * during initialization, and other modules can call them without direct imports.
 */

import type { PostNode } from "../api/posts.ts";

/**
 * Handler function types.
 */
export type SelectPostHandler = (post: PostNode) => void;
export type ReorderHandler = (
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
) => Promise<void>;
export type ReparentHandler = (
  uuid: string,
  newParentId: string | null,
  position: number,
) => Promise<void>;
export type RenderPostListHandler = () => void;

/**
 * Handler registry - populated during initialization.
 */
const handlers = {
  selectPost: null as SelectPostHandler | null,
  reorder: null as ReorderHandler | null,
  reparent: null as ReparentHandler | null,
  renderPostList: null as RenderPostListHandler | null,
};

/**
 * Register all handlers at once during initialization.
 */
export function registerHandlers(config: {
  selectPost: SelectPostHandler;
  reorder: ReorderHandler;
  reparent: ReparentHandler;
  renderPostList: RenderPostListHandler;
}): void {
  handlers.selectPost = config.selectPost;
  handlers.reorder = config.reorder;
  handlers.reparent = config.reparent;
  handlers.renderPostList = config.renderPostList;
}

/**
 * Get the select post handler.
 * @throws if handlers not initialized
 */
export function getSelectPostHandler(): SelectPostHandler {
  if (!handlers.selectPost) {
    throw new Error("Handlers not initialized - call registerHandlers first");
  }
  return handlers.selectPost;
}

/**
 * Get the reorder handler.
 * @throws if handlers not initialized
 */
export function getReorderHandler(): ReorderHandler {
  if (!handlers.reorder) {
    throw new Error("Handlers not initialized - call registerHandlers first");
  }
  return handlers.reorder;
}

/**
 * Get the reparent handler.
 * @throws if handlers not initialized
 */
export function getReparentHandler(): ReparentHandler {
  if (!handlers.reparent) {
    throw new Error("Handlers not initialized - call registerHandlers first");
  }
  return handlers.reparent;
}

/**
 * Get the render post list handler.
 * @throws if handlers not initialized
 */
export function getRenderPostListHandler(): RenderPostListHandler {
  if (!handlers.renderPostList) {
    throw new Error("Handlers not initialized - call registerHandlers first");
  }
  return handlers.renderPostList;
}

/**
 * Call renderPostList if registered (safe to call before initialization).
 */
export function callRenderPostList(): void {
  handlers.renderPostList?.();
}
