/**
 * Session key storage for encryption.
 *
 * Stores the Master Encryption Key (MEK) in memory only.
 * Key is cleared on logout or tab close.
 *
 * Encryption state:
 * - encryptionEnabled: User has set up encryption (PRF salt exists)
 * - sessionEncryptionKey: Derived key for this session (set after unlock)
 * - prfSalt: Salt used for PRF during passkey auth
 */

let sessionEncryptionKey: CryptoKey | null = null;
let prfSalt: string | null = null;
let encryptionEnabled = true;

// --- Initialization ---

/**
 * Initialize encryption with the PRF salt.
 * Call this when encryption settings indicate encryption is enabled.
 */
export function initEncryption(salt: string): void {
  prfSalt = salt;
  encryptionEnabled = true;
}

/**
 * Mark encryption as disabled.
 * Call this when encryption settings indicate encryption is not enabled.
 */
export function disableEncryption(): void {
  encryptionEnabled = false;
  prfSalt = null;
}

// --- Session Key ---

/**
 * Store the derived encryption key for this session.
 */
export function setSessionEncryptionKey(key: CryptoKey): void {
  sessionEncryptionKey = key;
}

/**
 * Get the current session encryption key.
 * Returns null if not unlocked.
 */
export function getSessionEncryptionKey(): CryptoKey | null {
  return sessionEncryptionKey;
}

/**
 * Clear the session encryption key (on logout).
 */
export function clearSessionEncryptionKey(): void {
  sessionEncryptionKey = null;
}

// --- PRF Salt ---

/**
 * Get the stored PRF salt.
 */
export function getPrfSalt(): string | null {
  return prfSalt;
}

// --- State Queries ---

/**
 * Check if encryption is enabled for this user.
 */
export function isEncryptionEnabled(): boolean {
  return encryptionEnabled;
}

/**
 * Check if the user needs to unlock (has encryption but no key in session).
 */
export function needsUnlock(): boolean {
  return encryptionEnabled && sessionEncryptionKey === null;
}

/**
 * Check if the session is unlocked (has encryption key).
 */
export function isUnlocked(): boolean {
  return sessionEncryptionKey !== null;
}

// --- Cleanup ---

/**
 * Clear all session state.
 */
export function clearAll(): void {
  sessionEncryptionKey = null;
  prfSalt = null;
  encryptionEnabled = false;
}

// --- Test Mode ---
// These functions are only available when built with TEST_MODE=1.
// The __TEST_MODE__ constant is replaced at build time by Vite.

declare const __TEST_MODE__: boolean;

/**
 * Check if a test encryption key has been injected.
 * Only available in test builds (TEST_MODE=1).
 * Returns null in production builds (code is tree-shaken).
 */
export function getInjectedTestKey(): string | null {
  if (__TEST_MODE__) {
    return (
      (window as unknown as { __TEST_ENCRYPTION_KEY__?: string })
        .__TEST_ENCRYPTION_KEY__ ?? null
    );
  }
  return null;
}

/**
 * Initialize encryption for test mode (no PRF salt, key injected directly).
 * Only available in test builds (TEST_MODE=1).
 */
export function initEncryptionForTest(): void {
  if (__TEST_MODE__) {
    encryptionEnabled = true;
    prfSalt = null; // Not needed in test mode
  }
}
