/**
 * Posts management - state, UI, and interactions.
 */

// State (re-export from state/ folder)
export * from "./state/index.ts";

// Handler registry
export { registerHandlers } from "./handlers.ts";

// Save and encryption
export { flushSave, saveBeacon, scheduleAutosave } from "./save.ts";

// Rendering
export { renderPostList } from "./render.ts";

// Editor setup
export { setupEditor, destroyEditor } from "./editor.ts";

// Selection
export { selectPost } from "./selection.ts";

// Actions
export {
  handleNewPost,
  handleDeletePost,
  handleDeletePostByNode,
} from "./actions.ts";

// Loading
export { loadPosts, loadPostsWithoutSelection } from "./load.ts";

// Reactive subscriptions
export { initSubscriptions } from "./subscriptions.ts";
