# App Frontend (JWT Protected)

## Structure

- `src/main.ts` - App entry point
- `src/setup-encryption.ts` - Encryption setup page
- `src/websocket.ts` - WebSocket client with auto-reconnect
- `src/api/` - API clients
- `src/crypto/` - Encryption utilities
- `src/editor/` - CodeMirror 6 editor
- `src/posts/` - Posts management
- `src/unlock/` - Passkey unlock modal

## Crypto Module

- `operations.ts` - WebCrypto AES-GCM, HKDF, base64url
- `keystore.ts` - Session encryption key storage (in-memory only)
- `post-encryption.ts` - Encrypt/decrypt posts

Key functions: `deriveEncryptionKeyFromPrf()`, `encryptContent()`, `decryptContent()`, `encryptBinary()`, `decryptBinary()`

## Editor Module

- `setup.ts` - Editor initialization, theme, and content reset
- `attachment-widget.ts` - Inline image widget with thumbnail-first display
- `checkbox-widget.ts` - Interactive checkboxes
- `slash-commands.ts` - Command palette

Key functions:
- `createEditor()` - Create new editor instance
- `resetEditorContent()` - Reset editor with new content (reuses DOM, clears undo history)

## Posts Module Structure

The posts module is organized into focused submodules:

```
src/posts/
├── state/              # State management (split by concern)
│   ├── signals.ts      # Reactive signals (editor, posts, loadedPost, isDirty)
│   ├── tree.ts         # Tree traversal & manipulation
│   ├── ui-state.ts     # Non-reactive state (titles, expanded, save timers)
│   ├── loading.ts      # Loading lock with withLoadingLock() helper
│   └── index.ts        # Barrel export
├── editor.ts           # Editor setup & lifecycle (setupEditor, destroyEditor)
├── selection.ts        # Post selection logic
├── save.ts             # Encryption & save logic (consolidated)
├── actions.ts          # CRUD operations
├── render.ts           # Post list rendering
├── drag-and-drop.ts    # Drag & drop reordering
├── load.ts             # App initialization
├── subscriptions.ts    # Reactive UI subscriptions
├── handlers.ts         # Handler registry (breaks circular deps)
├── types.ts            # Type definitions
└── index.ts            # Public API
```

### Key Patterns

**Loading Lock**: Use `withLoadingLock()` for async operations that should block other post operations:
```typescript
await withLoadingLock(async () => {
  // Editor is read-only during this block
  // Other selectPost/handleNewPost calls will be ignored
});
```

**State Access**: Import from `./state/index.ts` (or `./state/` for short):
```typescript
import { getLoadedPost, setIsDirty, withLoadingLock } from "./state/index.ts";
```

## Posts Behavior

- Auto-create first post on login with no posts
- After deleting last post, create new one automatically
- After deleting a post with others, select first remaining
- Editor is reused across post switches (not destroyed/recreated)
- Loading lock prevents concurrent post operations
