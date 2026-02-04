/**
 * State management for posts module.
 *
 * Re-exports all state-related functions organized by concern:
 * - signals: Reactive state (editor, posts, loadedPost, isDirty, etc.)
 * - tree: Tree traversal and manipulation
 * - ui-state: Non-reactive UI state (expanded helpers, save timers)
 * - loading: Loading lock for async operations
 */

// Reactive signals - use .get(), .set(), .update(), .subscribe()
export {
  type SyncStatus,
  editorSignal,
  postsSignal,
  loadedPostSignal,
  loadedDecryptedContentSignal,
  isDirtySignal,
  syncStatusSignal,
  decryptedTitlesSignal,
  expandedPostsSignal,
  expandedChangedSignal,
  pendingEncryptedDataSignal,
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
export type { PendingEncryptedData } from "./ui-state.ts";
export {
  getLastSelectedPostUuid,
  setLastSelectedPostUuid,
  clearLastSelectedPostUuid,
  isExpanded,
  toggleExpanded,
  setExpanded,
  expandToDepth,
  setSaveTimeout,
  clearSaveTimeout,
} from "./ui-state.ts";

// Loading state
export { isLoading, setLoading, withLoadingLock } from "./loading.ts";
