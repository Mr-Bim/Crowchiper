import { fetchWithAuth } from "./auth.ts";
import { getErrorMessage } from "./utils.ts";

declare const API_PATH: string;

export interface EncryptionSettings {
  setup_done: boolean;
  encryption_enabled: boolean;
  prf_salt?: string;
}

export async function getEncryptionSettings(): Promise<EncryptionSettings> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/settings`);
  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to get encryption settings",
    );
    throw new Error(errorMsg);
  }
  return response.json();
}

export interface SetupResponse {
  prf_salt: string;
}

export async function setupEncryption(): Promise<SetupResponse> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/setup`, {
    method: "POST",
  });
  if (response.status === 409) {
    // Already set up - throw a specific error so caller can handle
    throw new ConflictError("Encryption already set up");
  }
  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to setup encryption",
    );
    throw new Error(errorMsg);
  }
  return response.json();
}

export async function skipEncryption(): Promise<void> {
  const response = await fetchWithAuth(`${API_PATH}/encryption/skip`, {
    method: "POST",
  });
  if (response.status === 409) {
    // Already set up - throw a specific error so caller can handle
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
