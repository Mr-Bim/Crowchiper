/**
 * Cryptographic operations for end-to-end encryption.
 *
 * Uses WebCrypto API with AES-256-GCM for encryption and HKDF-SHA256
 * for key derivation from PRF output.
 */

// Encrypted content format version
export const ENCRYPTED_FORMAT_VERSION = 1;

export interface EncryptedData {
  ciphertext: string; // base64url encoded
  iv: string; // base64url encoded
}

/**
 * Derive a 256-bit AES key from PRF output using HKDF-SHA256.
 */
export async function deriveEncryptionKeyFromPrf(
  prfOutput: ArrayBuffer,
): Promise<CryptoKey> {
  // Import PRF output as raw key material for HKDF
  const keyMaterial = await crypto.subtle.importKey(
    "raw",
    prfOutput,
    "HKDF",
    false,
    ["deriveKey"],
  );

  // Derive AES-256-GCM key using HKDF
  const salt = new TextEncoder().encode("crowchiper-encryption-key-v1");
  const info = new TextEncoder().encode("posts-encryption-key");

  return crypto.subtle.deriveKey(
    {
      name: "HKDF",
      salt,
      info,
      hash: "SHA-256",
    },
    keyMaterial,
    {
      name: "AES-GCM",
      length: 256,
    },
    false, // not extractable
    ["encrypt", "decrypt"],
  );
}

/**
 * Extract PRF output from a WebAuthn credential response.
 * Returns null if PRF extension was not used or not supported.
 */
export function extractPrfOutput(
  credential: PublicKeyCredential | { clientExtensionResults?: unknown },
): ArrayBuffer | null {
  const extensions = (
    credential as {
      clientExtensionResults?: {
        prf?: { results?: { first?: ArrayBuffer } };
      };
    }
  ).clientExtensionResults;

  return extensions?.prf?.results?.first ?? null;
}

/**
 * Encrypt content using AES-256-GCM.
 * Returns the ciphertext and IV as base64url-encoded strings.
 */
export async function encryptContent(
  content: string,
  key: CryptoKey,
): Promise<EncryptedData> {
  // Generate random 12-byte IV
  const iv = crypto.getRandomValues(new Uint8Array(12));

  // Encode content as UTF-8
  const plaintext = new TextEncoder().encode(content);

  // Encrypt
  const ciphertext = await crypto.subtle.encrypt(
    {
      name: "AES-GCM",
      iv,
    },
    key,
    plaintext,
  );

  return {
    ciphertext: arrayBufferToBase64Url(ciphertext),
    iv: arrayBufferToBase64Url(iv.buffer),
  };
}

/**
 * Decrypt content using AES-256-GCM.
 * @param ciphertext - base64url-encoded ciphertext
 * @param iv - base64url-encoded initialization vector
 * @param encryptionVersion - the encryption format version
 * @param key - the decryption key
 */
export async function decryptContent(
  ciphertext: string,
  iv: string,
  encryptionVersion: number,
  key: CryptoKey,
): Promise<string> {
  if (encryptionVersion !== ENCRYPTED_FORMAT_VERSION) {
    throw new Error(
      `Unsupported encryption format version: ${encryptionVersion}`,
    );
  }

  const ciphertextBuffer = base64UrlToArrayBuffer(ciphertext);
  const ivBuffer = base64UrlToArrayBuffer(iv);

  const plaintext = await crypto.subtle.decrypt(
    {
      name: "AES-GCM",
      iv: ivBuffer,
    },
    key,
    ciphertextBuffer,
  );

  return new TextDecoder().decode(plaintext);
}

// --- Base64URL utilities ---

function arrayBufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

function base64UrlToArrayBuffer(base64url: string): ArrayBuffer {
  // Add padding if needed
  let base64 = base64url.replace(/-/g, "+").replace(/_/g, "/");
  while (base64.length % 4 !== 0) {
    base64 += "=";
  }

  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

export function base64UrlToUint8Array(base64url: string): Uint8Array {
  return new Uint8Array(base64UrlToArrayBuffer(base64url));
}

/**
 * Import a raw AES-256 key from base64url-encoded bytes.
 * Used for test mode where the key is injected directly.
 */
export async function importRawKey(keyBase64Url: string): Promise<CryptoKey> {
  const keyBytes = base64UrlToArrayBuffer(keyBase64Url);
  return crypto.subtle.importKey(
    "raw",
    keyBytes,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

// --- Binary encryption utilities (for images) ---

export interface EncryptedBinaryData {
  ciphertext: ArrayBuffer;
  iv: string; // base64url encoded
}

/**
 * Encrypt binary data (e.g., images) using AES-256-GCM.
 * Returns the ciphertext as ArrayBuffer and IV as base64url string.
 */
export async function encryptBinary(
  data: ArrayBuffer,
  key: CryptoKey,
): Promise<EncryptedBinaryData> {
  // Generate random 12-byte IV
  const iv = crypto.getRandomValues(new Uint8Array(12));

  // Encrypt
  const ciphertext = await crypto.subtle.encrypt(
    {
      name: "AES-GCM",
      iv,
    },
    key,
    data,
  );

  return {
    ciphertext,
    iv: arrayBufferToBase64Url(iv.buffer),
  };
}

/**
 * Decrypt binary data using AES-256-GCM.
 * @param ciphertext - encrypted binary data
 * @param iv - base64url-encoded initialization vector
 * @param key - the decryption key
 */
export async function decryptBinary(
  ciphertext: ArrayBuffer,
  iv: string,
  key: CryptoKey,
): Promise<ArrayBuffer> {
  const ivBuffer = base64UrlToArrayBuffer(iv);

  return crypto.subtle.decrypt(
    {
      name: "AES-GCM",
      iv: ivBuffer,
    },
    key,
    ciphertext,
  );
}

/**
 * Convert ArrayBuffer to base64url string.
 * Exported for use in attachment upload.
 */
export function toBase64Url(buffer: ArrayBuffer): string {
  return arrayBufferToBase64Url(buffer);
}

/**
 * Convert base64url string to ArrayBuffer.
 * Exported for use in attachment decryption.
 */
export function fromBase64Url(base64url: string): ArrayBuffer {
  return base64UrlToArrayBuffer(base64url);
}
