/**
 * Posts management - state, UI, and interactions.
 */

// State (re-export from state/ folder)
export * from "./state/index.ts";

// Drag and drop
export { initDragAndDrop } from "./drag-and-drop.ts";

// Handler registry
export { registerHandlers } from "./handlers.ts";

// Save and encryption
export { handleSave, saveBeacon } from "./save.ts";

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
