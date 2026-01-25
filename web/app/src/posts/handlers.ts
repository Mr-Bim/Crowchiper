/**
 * Handler registry for post operations.
 *
 * This module breaks circular dependencies by providing a central place
 * to register and retrieve handlers. Modules register their handlers here
 * during initialization, and other modules can call them without direct imports.
 */

import type {
  PostHandlers,
  SelectPostHandler,
  ReorderHandler,
  ReparentHandler,
  RenderPostListHandler,
} from "./types.ts";

// Re-export handler types for convenience
export type {
  SelectPostHandler,
  ReorderHandler,
  ReparentHandler,
  RenderPostListHandler,
};

let handlersRegistered = false;

/**
 * Handler registry - populated during initialization.
 */
const handlers: {
  [K in keyof PostHandlers]: PostHandlers[K] | null;
} = {
  selectPost: null,
  reorder: null,
  reparent: null,
  renderPostList: null,
};

export function isHandlersRegistered() {
  return handlersRegistered;
}

/**
 * Register all handlers at once during initialization.
 */
export function registerHandlers(config: PostHandlers): void {
  handlersRegistered = true;
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
