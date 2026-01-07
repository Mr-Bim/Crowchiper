/**
 * Session key storage for encryption.
 *
 * Stores the Master Encryption Key (MEK) in memory only.
 * Key is cleared on logout or tab close.
 */

let sessionEncryptionKey: CryptoKey | null = null;
let prfSalt: string | null = null;
let encryptionEnabled = false;

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

/**
 * Store the PRF salt from encryption settings.
 */
export function setPrfSalt(salt: string): void {
	prfSalt = salt;
	encryptionEnabled = true;
}

/**
 * Get the stored PRF salt.
 */
export function getPrfSalt(): string | null {
	return prfSalt;
}

/**
 * Check if encryption is enabled for this user.
 */
export function isEncryptionEnabled(): boolean {
	return encryptionEnabled;
}

/**
 * Set encryption enabled state (for users without PRF).
 */
export function setEncryptionEnabled(enabled: boolean): void {
	encryptionEnabled = enabled;
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

/**
 * Clear all session state.
 */
export function clearAll(): void {
	sessionEncryptionKey = null;
	prfSalt = null;
}
