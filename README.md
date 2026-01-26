# Crowchiper

**Warning:** 
!Red flag begins here!

This is just a test project for me to see how far Claude Code could take me.

There are probably some glaring security holes in this app, and there's a lot of stuff to look into.

I have never written rust before, I am no security expert, so I feel the most bad about the frontend security atm.

I needed a notes app that could be served from a base-url, and I wanted to use the prf extension to encrypt my notes. 

!Red flag ends here!

A secure notes app with passkey authentication and optional end-to-end encryption. Ships as a single binary with an embedded frontend.

## Features

- **Passkey Authentication** - No passwords, just WebAuthn passkeys
- **End-to-End Encryption** - Optional client-side encryption using WebAuthn PRF extension
- **Single Binary** - Frontend embedded in the Rust binary, easy to deploy
- **SQLite Database** - No external database required
- **Reverse Proxy Support** - Configurable base path for deployment behind proxies

## Quick Start

```bash
# Build frontend
npm install
npm run build-all

# Build and run server
cargo run -- --port 7291 --database crowchiper.db
```

Visit `http://localhost:7291` to register and start using the app.

## CLI Options

```bash
crowchiper [OPTIONS]

Options:
  --port <PORT>           Server port (default: 7291)
  --database <PATH>       SQLite database path (default: crowchiper.db)
  --base <PATH>           Base path for reverse proxy (e.g., /app)
  --no-signup             Disable public registration
  --rp-id <DOMAIN>        WebAuthn Relying Party ID (default: localhost)
  --rp-origin <URL>       WebAuthn origin (default: http://localhost:7291)
  --create-admin          Create admin account on startup
```

### Examples

```bash
# Development
cargo run -- --port 7291 --database crowchiper.db

# Behind reverse proxy at /app
cargo run -- --base /app --rp-id example.com --rp-origin https://example.com

# Disable public signups
cargo run -- --no-signup --create-admin
```

## End-to-End Encryption

Crowchiper supports optional end-to-end encryption using the WebAuthn PRF extension. When enabled:

- Encryption keys are derived from your passkey - never stored on the server
- The server only sees encrypted content
- Encryption is transparent - just authenticate and use the app normally

**Requirements:**
- A browser that supports passkeys

Users without PRF support can skip encryption and use plaintext storage.

**Warning:** Losing your passkey means losing access to encrypted data. There is no recovery mechanism. (Yet)

## Development

```bash
# Install dependencies
npm install

# Build frontend for production
npm run build-all

# Run the server (JWT_SECRET must be 32+ characters)
JWT_SECRET=your-32-character-secret-here cargo run
```

### Common Commands

| Command | Description |
|---------|-------------|
| `npm run build-all` | Build frontend for production |
| `npm run build-all-test` | Build frontend with test mode |
| `npm run lint:fix` | TypeScript type check and lint fix |
| `cargo run -- --port 7291` | Run server on port 7291 |
| `cargo build --release` | Build release binary |

## Testing

```bash
# Install dependencies (if not already done)
npm install

# Build frontend and Rust in test mode
npm run prepare-test

# Run Rust tests
npm run test:rust

# Run Playwright e2e tests
npm run test:web

# Run all tests
npm run test:all
```

### First-Time Setup for E2E Tests

```bash
# Install Playwright browser
npx playwright install chromium
```

## License
