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
  type SyncStatus,
  editorSignal,
  postsSignal,
  loadedPostSignal,
  loadedDecryptedContentSignal,
  isDirtySignal,
  syncStatusSignal,
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
  getSyncStatus,
  setSyncStatus,
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
  getLastSelectedPostUuid,
  setLastSelectedPostUuid,
  clearLastSelectedPostUuid,
  getDecryptedTitles,
  setDecryptedTitles,
  setDecryptedTitle,
  getDecryptedTitle,
  isExpanded,
  toggleExpanded,
  setExpanded,
  expandToDepth,
  expandedChangedSignal,
  getPendingEncryptedData,
  setPendingEncryptedData,
  setSaveTimeout,
  clearSaveTimeout,
} from "./ui-state.ts";

// Loading state
export { isLoading, setLoading, withLoadingLock } from "./loading.ts";
