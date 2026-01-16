/**
 * Encryption setup flow.
 *
 * Simple flow:
 * 1. Test passkey for PRF support
 * 2. If PRF supported: call setup API (server generates salt), redirect to app
 * 3. If PRF not supported: show message, call skip API, redirect to app (no encryption)
 */

import {
  type AuthenticationResponseJSON,
  startAuthentication,
} from "@simplewebauthn/browser";
import {
  ConflictError,
  getEncryptionSettings,
  setupEncryption,
  skipEncryption,
} from "./api/encryption-settings.ts";
import { getErrorMessage } from "./api/utils.ts";
import { extractPrfOutput } from "./crypto/operations.ts";

declare const API_PATH: string;
declare const APP_PATH: string;
declare const LOGIN_PATH: string;
declare const __RELEASE_MODE__: boolean;

// --- DOM Elements ---

function getElements() {
  return {
    stepPrfTest: document.getElementById("step-prf-test") as HTMLDivElement,
    stepPrfSupported: document.getElementById(
      "step-prf-supported",
    ) as HTMLDivElement,
    stepNoPrf: document.getElementById("step-no-prf") as HTMLDivElement,
    testPrfBtn: document.getElementById("test-prf-btn") as HTMLButtonElement,
    skipBtn: document.getElementById("skip-btn") as HTMLButtonElement,
    enableEncryptionBtn: document.getElementById(
      "enable-encryption-btn",
    ) as HTMLButtonElement,
    skipEncryptionBtn: document.getElementById(
      "skip-encryption-btn",
    ) as HTMLButtonElement,
    prfError: document.getElementById("prf-error") as HTMLDivElement,
    enableError: document.getElementById("enable-error") as HTMLDivElement,
    continueLink: document.getElementById("continue-link") as HTMLAnchorElement,
  };
}

// --- PRF Testing ---

/**
 * Test if the user's passkey supports PRF by attempting authentication with PRF extension.
 * Returns true if PRF is supported, false otherwise.
 */
async function testPrfSupport(username?: string): Promise<boolean> {
  // Get authentication options from server
  const optionsResponse = await fetch(`${API_PATH}/passkeys/login/start`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: username || null }),
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

  // Add PRF extension to test if it's supported
  // We use an eval salt for testing - we just want to check if PRF works
  const testSalt = new Uint8Array(32);
  crypto.getRandomValues(testSalt);

  const optionsWithPrf = {
    ...response.publicKey,
    extensions: {
      ...response.publicKey.extensions,
      prf: {
        eval: {
          first: testSalt,
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
    await deleteChallenge(session_id);
    throw error;
  }

  // Complete authentication to consume the challenge
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

  // Check if PRF extension returned a result (also checks for injected test PRF)
  const prfOutput = extractPrfOutput(credential);
  return prfOutput != null;
}

async function deleteChallenge(sessionId: string): Promise<void> {
  try {
    await fetch(`${API_PATH}/passkeys/login/challenge/${sessionId}`, {
      method: "DELETE",
    });
  } catch {
    // Ignore errors - cleanup is best-effort
  }
}

// --- UI State ---

function showError(elements: ReturnType<typeof getElements>, message: string) {
  elements.prfError.textContent = message;
  elements.prfError.hidden = false;
}

function hideError(elements: ReturnType<typeof getElements>) {
  elements.prfError.hidden = true;
  elements.prfError.textContent = "";
}

function showPrfSupportedStep(elements: ReturnType<typeof getElements>) {
  elements.stepPrfTest.hidden = true;
  elements.stepPrfSupported.hidden = false;
}

function showNoPrfStep(elements: ReturnType<typeof getElements>) {
  elements.stepPrfTest.hidden = true;
  elements.stepNoPrf.hidden = false;
}

function setLoading(
  elements: ReturnType<typeof getElements>,
  loading: boolean,
) {
  elements.testPrfBtn.disabled = loading;
  elements.skipBtn.disabled = loading;
  if (loading) {
    elements.testPrfBtn.textContent = "Testing...";
  } else {
    elements.testPrfBtn.textContent = "Test Passkey";
  }
}

// --- Initialization ---

async function init(): Promise<void> {
  const elements = getElements();

  // Update continue link with correct app path
  elements.continueLink.href = APP_PATH;

  // Check if user already has encryption set up
  try {
    const settings = await getEncryptionSettings();
    if (settings.setup_done) {
      // Already set up, redirect to app
      window.location.href = APP_PATH;
      return;
    }
  } catch (error) {
    // If we can't get settings, user might not be logged in
    console.error("Failed to get encryption settings:", error);
    window.location.href = LOGIN_PATH;
    return;
  }

  // Wire up event handlers
  elements.testPrfBtn.addEventListener("click", () => handleTestPrf(elements));
  elements.skipBtn.addEventListener("click", () => handleSkip(elements));
  elements.enableEncryptionBtn.addEventListener("click", () =>
    handleEnableEncryption(elements),
  );
  elements.skipEncryptionBtn.addEventListener("click", () =>
    handleSkipEncryption(elements),
  );
}

async function handleTestPrf(
  elements: ReturnType<typeof getElements>,
): Promise<void> {
  hideError(elements);
  setLoading(elements, true);

  try {
    // In test mode, check for injected username (Chrome's virtual authenticator
    // doesn't support discoverable credentials)
    let username: string | undefined;
    if (!__RELEASE_MODE__) {
      username =
        (window as unknown as { __TEST_USERNAME__?: string })
          .__TEST_USERNAME__ ?? undefined;
    }

    const prfSupported = await testPrfSupport(username);

    if (prfSupported) {
      // PRF is supported - show confirmation step
      showPrfSupportedStep(elements);
    } else {
      // PRF not supported - show the no-PRF step
      showNoPrfStep(elements);

      // Mark encryption as skipped
      try {
        await skipEncryption();
      } catch (error) {
        if (error instanceof ConflictError) {
          // Already set up, redirect to app
          window.location.href = APP_PATH;
          return;
        }
        throw error;
      }
    }
  } catch (error) {
    console.error("PRF test failed:", error);
    // Don't show error for user abort
    if (error instanceof Error && error.name === "NotAllowedError") {
      // User cancelled - reset state
      setLoading(elements, false);
      return;
    }
    showError(
      elements,
      error instanceof Error ? error.message : "Failed to test passkey",
    );
    setLoading(elements, false);
  }
}

async function handleSkip(
  elements: ReturnType<typeof getElements>,
): Promise<void> {
  hideError(elements);
  setLoading(elements, true);

  try {
    await skipEncryption();
    // Redirect to app
    window.location.href = APP_PATH;
  } catch (error) {
    console.error("Skip encryption failed:", error);
    if (error instanceof ConflictError) {
      // Already set up, redirect to app
      window.location.href = APP_PATH;
      return;
    }
    showError(
      elements,
      error instanceof Error ? error.message : "Failed to skip encryption",
    );
    setLoading(elements, false);
  }
}

async function handleEnableEncryption(
  elements: ReturnType<typeof getElements>,
): Promise<void> {
  elements.enableError.hidden = true;
  elements.enableEncryptionBtn.disabled = true;
  elements.skipEncryptionBtn.disabled = true;
  elements.enableEncryptionBtn.textContent = "Enabling...";

  try {
    await setupEncryption();
    // Success - redirect to app
    window.location.href = APP_PATH;
  } catch (error) {
    console.error("Enable encryption failed:", error);
    if (error instanceof ConflictError) {
      // Already set up, redirect to app
      window.location.href = APP_PATH;
      return;
    }
    elements.enableError.textContent =
      error instanceof Error ? error.message : "Failed to enable encryption";
    elements.enableError.hidden = false;
    elements.enableEncryptionBtn.disabled = false;
    elements.skipEncryptionBtn.disabled = false;
    elements.enableEncryptionBtn.textContent = "Enable Encryption";
  }
}

async function handleSkipEncryption(
  elements: ReturnType<typeof getElements>,
): Promise<void> {
  elements.enableError.hidden = true;
  elements.enableEncryptionBtn.disabled = true;
  elements.skipEncryptionBtn.disabled = true;

  try {
    await skipEncryption();
    // Redirect to app
    window.location.href = APP_PATH;
  } catch (error) {
    console.error("Skip encryption failed:", error);
    if (error instanceof ConflictError) {
      // Already set up, redirect to app
      window.location.href = APP_PATH;
      return;
    }
    elements.enableError.textContent =
      error instanceof Error ? error.message : "Failed to skip encryption";
    elements.enableError.hidden = false;
    elements.enableEncryptionBtn.disabled = false;
    elements.skipEncryptionBtn.disabled = false;
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  await init();
});
