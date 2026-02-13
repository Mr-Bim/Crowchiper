# API Endpoints

Routes at `{base}/api/`.

## Users (`/api/users`)
- `POST /api/users/` - Create user (disabled with `--no-signup`)
- `DELETE /api/users/{uuid}` - Delete user (self or admin)

## Passkeys (`/api/passkeys`)
- `POST /api/passkeys/register/start` - Start registration
- `POST /api/passkeys/register/finish` - Complete registration
- `POST /api/passkeys/login/start` - Start login
- `POST /api/passkeys/login/finish` - Complete login (sets JWT cookies)
- `DELETE /api/passkeys/login/challenge/{session_id}` - Cancel challenge
- `POST /api/passkeys/claim/start` - Start account reclaim
- `POST /api/passkeys/claim/finish` - Complete reclaim

## Posts (`/api/posts`)
- `GET /api/posts/` - List posts tree (with depth parameter)
- `POST /api/posts/` - Create post
- `GET /api/posts/{uuid}` - Get post
- `PUT /api/posts/{uuid}` - Update post (optional `attachment_uuids`)
- `POST /api/posts/{uuid}` - Update post (sendBeacon on page unload)
- `DELETE /api/posts/{uuid}` - Delete post
- `GET /api/posts/{uuid}/children` - List children of a post
- `POST /api/posts/{uuid}/move` - Move post to new parent/position
- `POST /api/posts/reorder` - Reorder posts within a parent

## Tokens (`/api/tokens`)
- `GET /api/tokens/` - List active refresh tokens for current user
- `DELETE /api/tokens/` - Revoke all refresh tokens for current user (logout everywhere)
- `GET /api/tokens/verify` - Verify access token is valid
- `POST /api/tokens/logout` - Logout (revoke refresh token, clear cookies)
- `DELETE /api/tokens/{jti}` - Revoke specific refresh token

## Encryption (`/api/encryption`)
- `POST /api/encryption/setup` - Initial encryption setup (generates PRF salt)
- `POST /api/encryption/skip` - Skip encryption (PRF not supported)

## Attachments (`/api/attachments`)
- `POST /api/attachments/` - Upload (multipart: image, image_iv, thumbnail, thumbnail_iv, encryption_version)
- `GET /api/attachments/{uuid}` - Get image (binary + `X-Encryption-IV` header)
- `GET /api/attachments/{uuid}/thumbnails` - Get all thumbnails (multipart)
- `GET /api/attachments/{uuid}/thumbnail/{size}` - Get specific thumbnail (sm/md/lg)

## Admin (`/api/admin`) - admin only
- `GET /api/admin/users` - List all activated users

## User Settings (`/api/user`)
- `GET /api/user/settings` - Get user settings (encryption config + admin info)

## Config (`/api/config`) - public
- `GET /api/config/` - Public config (no_signup, authenticated status, version, git hash)

## Test (`/api/test`) - test-mode only
- `POST /api/test/admin` - Create admin user for testing
- `POST /api/test/token` - Store token in database for testing
- `POST /api/test/generate-tokens` - Generate JWT tokens for testing

## Validation

Username: non-empty, max 32 chars, alphanumeric + underscore only.
