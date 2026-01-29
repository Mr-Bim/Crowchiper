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

const DEV_KEY_STORAGE_KEY = "dev-encryption-key";

// Vite sets import.meta.env.DEV to true in dev mode
const IS_DEV = import.meta.env.DEV;

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
 * In test mode, also caches the key in sessionStorage for dev hot-reload.
 */
export async function setSessionEncryptionKey(key: CryptoKey): Promise<void> {
  sessionEncryptionKey = key;

  // In dev mode, cache the key in sessionStorage to persist across reloads
  if (IS_DEV) {
    try {
      const exported = await crypto.subtle.exportKey("raw", key);
      const base64 = btoa(String.fromCharCode(...new Uint8Array(exported)));
      sessionStorage.setItem(DEV_KEY_STORAGE_KEY, base64);
    } catch {
      // Key might not be extractable, ignore
    }
  }
}

/**
 * Get the current session encryption key.
 * Returns null if not unlocked.
 */
export function getSessionEncryptionKey(): CryptoKey | null {
  return sessionEncryptionKey;
}

/**
 * Try to restore the encryption key from sessionStorage (test mode only).
 * Returns true if key was restored successfully.
 */
export async function tryRestoreDevKey(): Promise<boolean> {
  if (!IS_DEV) return false;

  const cached = sessionStorage.getItem(DEV_KEY_STORAGE_KEY);
  if (!cached) return false;

  try {
    const bytes = Uint8Array.from(atob(cached), (c) => c.charCodeAt(0));
    const key = await crypto.subtle.importKey(
      "raw",
      bytes,
      { name: "AES-GCM", length: 256 },
      true, // extractable for dev caching
      ["encrypt", "decrypt"],
    );
    sessionEncryptionKey = key;
    return true;
  } catch {
    sessionStorage.removeItem(DEV_KEY_STORAGE_KEY);
    return false;
  }
}

/**
 * Clear the session encryption key (on logout).
 */
export function clearSessionEncryptionKey(): void {
  sessionEncryptionKey = null;
  if (IS_DEV) {
    sessionStorage.removeItem(DEV_KEY_STORAGE_KEY);
  }
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
