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

- `setup.ts` - Editor initialization and theme
- `attachment-widget.ts` - Inline image widget with thumbnail-first display
- `checkbox-widget.ts` - Interactive checkboxes
- `slash-commands.ts` - Command palette

## Posts Behavior

- Auto-create first post on login with no posts
- After deleting last post, create new one automatically
- After deleting a post with others, select first remaining
