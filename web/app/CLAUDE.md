# App Frontend (JWT Protected)

## Structure

- `src/main.ts` - App entry point
- `src/setup-encryption.ts` - Encryption setup page
- `src/websocket.ts` - WebSocket client with auto-reconnect
- `src/reactive.ts` - Minimal signal implementation
- `src/toast.ts` - Toast notifications
- `src/spellcheck.ts` - Spellcheck toggle
- `src/settings-panel.ts` - Session management UI (token list, logout). Uses `escapeHtml()` for all user-controlled data (IPs, JTIs) rendered via innerHTML.
- `src/api/` - API clients
- `src/crypto/` - Encryption utilities
- `src/editor/` - CodeMirror 6 editor
- `src/posts/` - Posts management
- `src/shared/` - App-specific shared code (attachment utils, image cache)
- `src/unlock/` - Passkey unlock modal

## Crypto Module

- `operations.ts` - WebCrypto AES-GCM, HKDF, base64url
- `keystore.ts` - Session encryption key storage (in-memory only)
- `post-encryption.ts` - Encrypt/decrypt posts

Key functions: `deriveEncryptionKeyFromPrf()`, `encryptContent()`, `decryptContent()`, `encryptBinary()`, `decryptBinary()`

## Editor Module

- `setup.ts` - Editor initialization, theme, and content reset
- `checkbox-widget.ts` - Interactive checkboxes
- `slash-commands.ts` - Command palette
- `date-shortcuts.ts` - Date shortcut expansion
- `compress-worker.ts` - Image compression web worker
- `heic-convert.ts` - HEIC conversion (lazy-loaded)
- `attachment-widget/` - Inline image widget subsystem (see below)

Key functions:
- `createEditor()` - Create new editor instance
- `resetEditorContent()` - Reset editor with new content (reuses DOM, clears undo history)

### Attachment Widget (`editor/attachment-widget/`)

Image display and upload subsystem with gallery support:

- `index.ts` - Public exports
- `widget.ts` - CodeMirror widget for inline images
- `widget-upload.ts` - Upload widget with progress display
- `upload.ts` - Multi-stage upload logic (converting/compressing/encrypting/uploading)
- `decorations.ts` - Editor decorations
- `gallery-helpers.ts` - Gallery insertion/deletion helpers
- `patterns.ts` - Gallery syntax parsing (`::gallery{}..::`)
- `progress.ts` - Upload progress tracking
- `thumbnail.ts` - Thumbnail display with lazy loading
- `lightbox.ts` - Full-size image lightbox
- `cache.ts` - Thumbnail/image caching
- `keyboard.ts` - Keyboard navigation within galleries
- `types.ts` - Type definitions
- `utils.ts` - Shared utilities

## Posts Module Structure

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
