# Frontend

## Structure

- `login/` - Login/register/claim pages (no auth required)
- `app/` - JWT-protected app pages
- `dashboard/` - JWT-protected admin dashboard
- `inline/inline.ts` - Shared JS injected into all pages (path constants, theme toggle)
- `shared/` - Cross-app shared utilities
- `styles.css` - Global styles

## Build System

Three Vite builds (selected via `BUILD` env var):
1. `web/login/` -> `dist/login/` (base `/login`)
2. `web/app/` -> `dist/app/` (base from `config.assets`)
3. `web/dashboard/` -> `dist/dashboard/` (base `/dashboard`)

Dev mode: unified dev server on port 5173 serving all apps.

CSS < 20KB is inlined, `styles.css` stays external.

## Shared Utilities (`shared/`)

- `api-utils.ts` - Fetch helpers (JSON requests, error handling)
- `dom.ts` - Type-safe `getRequiredElement()`, `getOptionalElement()`, and `escapeHtml()` for XSS prevention
- `storage.ts` - Type-safe localStorage wrapper with Valibot validation (theme, spellcheck)
- `icons/` - SVG icon exports (check, chevron-down, close, translate)

## WebAuthn Frontend

Uses `@simplewebauthn/browser`. Always pass `options.publicKey` to `startRegistration()` (backend returns wrapped object).

## Encryption

Client-side E2E encryption using WebAuthn PRF extension. Key derived from PRF output via HKDF, never stored.

**Encrypted content format**: `{ "v": 1, "ct": "<base64url>", "iv": "<base64url>" }`

**PRF support**: Chrome/Edge (full), Safari 18+ (iCloud only). Users without PRF skip encryption.

## Posts Save Behavior

1. Local encryption: 1s debounce
2. Server save: 60s interval
3. Navigation: immediate save
4. Browser close: `sendBeacon` with pending data (eager encryption for pagehide safety)

## Image Attachments

Gallery syntax: `::gallery{}![alt](attachment:<uuid>)::`

Reference counting: attachments start at 0, incremented on post save, deleted when count reaches 0.

Size limits: 10MB image, 100KB thumbnail, 400px max dimensions.
