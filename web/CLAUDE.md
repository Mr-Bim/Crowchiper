# Frontend

## Structure

- `public/` - Login/register pages (no auth required)
- `app/` - JWT-protected app pages
- `inline/inline.ts` - Shared JS injected into all pages
- `styles.css` - Global styles

## Build System

Two Vite builds:
1. `web/public/` -> `dist/login/` (base `/login`)
2. `web/app/` -> `dist/app/` (base from `config.assets`)

CSS < 20KB is inlined, `styles.css` stays external.

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
4. Browser close: `sendBeacon` with pending data

## Image Attachments

Gallery syntax: `::gallery{}![alt](attachment:<uuid>)::`

Reference counting: attachments start at 0, incremented on post save, deleted when count reaches 0.

Size limits: 10MB image, 100KB thumbnail, 400px max dimensions.
