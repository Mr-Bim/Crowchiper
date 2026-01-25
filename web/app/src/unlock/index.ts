/**
 * Unlock flow for encrypted posts.
 *
 * Handles the passkey authentication to derive the encryption key.
 */

import {
  type AuthenticationResponseJSON,
  startAuthentication,
} from "@simplewebauthn/browser";
import { getErrorMessage } from "../api/utils.ts";
import {
  base64UrlToUint8Array,
  deriveEncryptionKeyFromPrf,
  extractPrfOutput,
} from "../crypto/operations.ts";
import { getPrfSalt, setSessionEncryptionKey } from "../crypto/keystore.ts";
import { getOptionalElement } from "../../../shared/dom.ts";

declare const API_PATH: string;
declare const __TEST_MODE__: boolean;

// --- UI Helpers ---

/**
 * Show the unlock overlay prompting for passkey authentication.
 */
export function showUnlockOverlay(): void {
  const overlay = getOptionalElement("unlock-overlay");
  if (overlay) {
    overlay.hidden = false;
  }
}

/**
 * Hide the unlock overlay after successful authentication.
 */
export function hideUnlockOverlay(): void {
  const overlay = getOptionalElement("unlock-overlay");
  if (overlay) {
    overlay.hidden = true;
  }
}

function showUnlockError(message: string): void {
  const errorEl = getOptionalElement("unlock-error");
  if (errorEl) {
    errorEl.textContent = message;
    errorEl.hidden = false;
  }
}

function hideUnlockError(): void {
  const errorEl = getOptionalElement("unlock-error");
  if (errorEl) {
    errorEl.hidden = true;
  }
}

function setUnlockLoading(loading: boolean): void {
  const btn = getOptionalElement("unlock-btn", HTMLButtonElement);
  if (btn) {
    btn.disabled = loading;
    btn.textContent = loading ? "Unlocking..." : "Unlock with Passkey";
  }
}

// --- Unlock Handler ---

export type OnUnlockSuccess = () => Promise<void>;

/**
 * Create the unlock button click handler.
 * @param onSuccess - Callback to run after successful unlock
 */
export function createUnlockHandler(
  onSuccess: OnUnlockSuccess,
): () => Promise<void> {
  return async () => {
    hideUnlockError();
    setUnlockLoading(true);

    try {
      const prfSalt = getPrfSalt();
      if (!prfSalt) {
        throw new Error("No PRF salt found");
      }

      // In test mode, check for injected username (Chrome's virtual authenticator
      // doesn't support discoverable credentials)
      let username: string | undefined;
      if (__TEST_MODE__) {
        username =
          (window as unknown as { __TEST_USERNAME__?: string })
            .__TEST_USERNAME__ ?? undefined;
      }

      // Get authentication options from server
      const optionsResponse = await fetch(`${API_PATH}/passkeys/login/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username }),
      });

      if (!optionsResponse.ok) {
        const errorMsg = await getErrorMessage(
          optionsResponse,
          "Failed to start authentication",
        );
        throw new Error(errorMsg);
      }

      const response = await optionsResponse.json();
      const { session_id } = response;

      // Add PRF extension with the stored salt
      const saltBytes = base64UrlToUint8Array(prfSalt);
      const optionsWithPrf = {
        ...response.publicKey,
        extensions: {
          ...response.publicKey.extensions,
          prf: {
            eval: {
              first: saltBytes,
            },
          },
        },
      };

      let credential: AuthenticationResponseJSON | null = null;
      try {
        credential = await startAuthentication({
          optionsJSON: optionsWithPrf,
        });
      } catch (error) {
        // Clean up the challenge on error
        await fetch(`${API_PATH}/passkeys/login/challenge/${session_id}`, {
          method: "DELETE",
        }).catch(() => {});
        throw error;
      }

      // Complete authentication
      const finishResponse = await fetch(`${API_PATH}/passkeys/login/finish`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ session_id, credential }),
      });

      if (!finishResponse.ok) {
        const errorMsg = await getErrorMessage(
          finishResponse,
          "Failed to complete authentication",
        );
        throw new Error(errorMsg);
      }

      // Extract PRF output and derive key
      const prfOutput = extractPrfOutput(credential);
      if (!prfOutput) {
        throw new Error("PRF not supported by this passkey");
      }

      const key = await deriveEncryptionKeyFromPrf(prfOutput);
      setSessionEncryptionKey(key);

      // Hide overlay and run success callback
      hideUnlockOverlay();
      await onSuccess();
    } catch (error) {
      console.error("Unlock failed:", error);
      if (error instanceof Error && error.name === "NotAllowedError") {
        // User cancelled
        setUnlockLoading(false);
        return;
      }
      showUnlockError(
        error instanceof Error ? error.message : "Failed to unlock",
      );
      setUnlockLoading(false);
    }
  };
}
