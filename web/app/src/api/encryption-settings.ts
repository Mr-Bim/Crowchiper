/**
 * Encryption settings API client.
 */

import { fetchWithAuth } from "./auth.ts";
import { getErrorMessage } from "./utils.ts";
import {
  EncryptionSettingsSchema,
  SetupResponseSchema,
  validate,
  type EncryptionSettings,
  type SetupResponse,
} from "./schemas.ts";

declare const API_PATH: string;

// Re-export types for convenience
export type { EncryptionSettings, SetupResponse };

/**
 * Get the current encryption settings for the user.
 */
export async function getEncryptionSettings(): Promise<EncryptionSettings> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/settings`);
  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to get encryption settings",
    );
    throw new Error(errorMsg);
  }
  const data = await response.json();
  return validate(EncryptionSettingsSchema, data, "encryption settings");
}

/**
 * Set up encryption for the user's account.
 * Generates and stores a PRF salt for key derivation.
 * @throws ConflictError if encryption is already set up
 */
export async function setupEncryption(): Promise<SetupResponse> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/setup`, {
    method: "POST",
  });
  if (response.status === 409) {
    throw new ConflictError("Encryption already set up");
  }
  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to setup encryption",
    );
    throw new Error(errorMsg);
  }
  const data = await response.json();
  return validate(SetupResponseSchema, data, "setup response");
}

/**
 * Skip encryption setup (use unencrypted storage).
 * @throws ConflictError if encryption is already set up
 */
export async function skipEncryption(): Promise<void> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/skip`, {
    method: "POST",
  });
  if (response.status === 409) {
    throw new ConflictError("Encryption already set up");
  }
  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to skip encryption",
    );
    throw new Error(errorMsg);
  }
}

export class ConflictError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ConflictError";
  }
}
