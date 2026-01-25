/**
 * State management for posts module.
 *
 * Re-exports all state-related functions organized by concern:
 * - signals: Reactive state (editor, posts, loadedPost, isDirty)
 * - tree: Tree traversal and manipulation
 * - ui-state: Non-reactive UI state (titles, expanded, save timers)
 * - loading: Loading lock for async operations
 */

// Reactive signals and their accessors
export {
  editorSignal,
  postsSignal,
  loadedPostSignal,
  loadedDecryptedContentSignal,
  isDirtySignal,
  getEditor,
  setEditor,
  getPosts,
  setPosts,
  getLoadedPost,
  setLoadedPost,
  getLoadedDecryptedContent,
  setLoadedDecryptedContent,
  getIsDirty,
  setIsDirty,
} from "./signals.ts";

// Tree operations
export {
  findPost,
  findParent,
  getSiblings,
  addPost,
  removePost,
  movePostInSiblings,
  movePostToParent,
  getSiblingUuids,
  setPostChildren,
  flattenPosts,
  getFirstSelectablePost,
} from "./tree.ts";

// Non-reactive UI state
export {
  type PendingEncryptedData,
  getDecryptedTitles,
  setDecryptedTitles,
  setDecryptedTitle,
  getDecryptedTitle,
  isExpanded,
  toggleExpanded,
  setExpanded,
  expandToDepth,
  getPendingEncryptedData,
  setPendingEncryptedData,
  setSaveTimeout,
  clearSaveTimeout,
  getServerSaveInterval,
  setServerSaveInterval,
  clearServerSaveInterval,
} from "./ui-state.ts";

// Loading state
export { isLoading, setLoading, withLoadingLock } from "./loading.ts";
