# API Endpoints

Routes at `{base}/api/`.

## Users
- `POST /api/users` - Claim username (disabled with `--no-signup`)
- `DELETE /api/users/{uuid}` - Delete user

## Passkeys
- `POST /api/passkeys/register/start` - Start registration
- `POST /api/passkeys/register/finish` - Complete registration
- `POST /api/passkeys/login/start` - Start login
- `POST /api/passkeys/login/finish` - Complete login (sets JWT cookie)
- `DELETE /api/passkeys/login/challenge/{session_id}` - Cancel challenge
- `POST /api/passkeys/claim/start` - Start account reclaim
- `POST /api/passkeys/claim/finish` - Complete reclaim

## Posts
- `GET /api/posts` - List posts
- `GET /api/posts/{id}` - Get post
- `POST /api/posts` - Create post
- `PUT /api/posts/{id}` - Update post (optional `attachment_uuids`)
- `DELETE /api/posts/{id}` - Delete post

## User Settings
- `GET /api/user/settings` - Get user settings (encryption + admin info)

## Encryption
- `POST /api/encryption/setup` - Initial setup
- `POST /api/encryption/skip` - Skip encryption

## Admin (admin-only)
- `GET /api/admin/users` - List all activated users

## Attachments
- `POST /api/attachments` - Upload (multipart: image, image_iv, thumbnail, thumbnail_iv, encryption_version)
- `GET /api/attachments/{uuid}` - Get image (binary + `X-Encryption-IV` header)
- `GET /api/attachments/{uuid}/thumbnail` - Get thumbnail

## Validation

Username: non-empty, max 32 chars, alphanumeric + underscore only.
