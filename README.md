# Crowchiper

**Warning:** 
This is just a test project for me to see how far Claude Code could take me.

There are might be some security holes in this app, and there's a lot of stuff to look into.
I have never written rust before.

I needed a notes app that could be served from a base-url, and I wanted to use the prf extension to encrypt my notes. 

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
```

### Required

| Option | Description |
|--------|-------------|
| `JWT_SECRET` env var | JWT signing secret (min 32 characters). Set as environment variable. |

### Optional with Defaults

| Option | Default | Description |
|--------|---------|-------------|
| `-p, --port <PORT>` | `7291` | Server port to listen on |
| `-d, --database <PATH>` | `crowchiper.db` | SQLite database path (created if missing) |
| `--rp-id <DOMAIN>` | `localhost` | WebAuthn Relying Party ID (domain name) |
| `--rp-origin <URL>` | `http://localhost:7291` | WebAuthn origin (must use HTTPS for non-localhost) |
| `-l, --log-format <FORMAT>` | `pretty` | Log format: `pretty`, `json`, `compact` |

### Optional Flags

| Option | Description |
|--------|-------------|
| `-b, --base <PATH>` | Base path prefix for reverse proxy (e.g., `/app`) |
| `--no-signup` | Disable public user registration |
| `--create-admin` | Create admin account on startup and print claim URL |
| `-i, --ip-header <HEADER>` | Extract client IP from header (requires reverse proxy) |
| `--csp-nonce` | Add random nonce to CSP headers (for Cloudflare compatibility) |
| `--jwt-secret-file <PATH>` | Read JWT secret from file instead of env var |

### Examples

```bash
# Development
cargo run -- --port 7291 --database crowchiper.db

# Behind reverse proxy at /app
cargo run -- --base /app --rp-id example.com --rp-origin https://example.com

# Disable public signups
cargo run -- --no-signup --create-admin

# Behind nginx with X-Forwarded-For
cargo run -- --ip-header x-forward-for --rp-origin https://example.com

# Behind Cloudflare
cargo run -- --ip-header cf-connecting-ip --rp-origin https://example.com
```

## Reverse Proxy Configuration

When running behind a reverse proxy, configure the `--ip-header` option to extract the real client IP address. This is important for security features like rate limiting and session validation.

### Supported IP Headers

| Option | Header | Description |
|--------|--------|-------------|
| `x-forward-for` | `X-Forwarded-For` | Standard proxy header (uses first IP in chain) |
| `cf-connecting-ip` | `CF-Connecting-IP` | Cloudflare's client IP header |
| `x-real-ip` | `X-Real-IP` | Nginx real IP header |
| `forward` | `Forwarded` | RFC 7239 standard header |

### Example Configurations

**Nginx:**
```nginx
location /app {
    proxy_pass http://127.0.0.1:7291;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header Host $host;
}
```
```bash
crowchiper --base /app --ip-header x-forward-for --rp-origin https://example.com
```

**Important:** Only use `--ip-header` when running behind a trusted reverse proxy. If exposed directly to the internet, clients could spoof their IP address by setting the header themselves.

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
