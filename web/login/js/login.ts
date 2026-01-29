import {
  type AuthenticationResponseJSON,
  startAuthentication,
} from "@simplewebauthn/browser";
import { fetchWithRetry, getErrorMessage } from "./api-utils.ts";
import { getOptionalElement, getRequiredElement } from "../../shared/dom.ts";

declare const API_PATH: string;
declare const APP_PATH: string;
declare const LOGIN_PATH: string;

async function checkPasskeysAvailable(): Promise<boolean> {
  // Check if WebAuthn is supported
  if (!window.PublicKeyCredential) {
    return false;
  }

  // Check if conditional mediation (autofill) is available
  if (
    typeof PublicKeyCredential.isConditionalMediationAvailable === "function"
  ) {
    const available =
      await PublicKeyCredential.isConditionalMediationAvailable();
    if (!available) {
      return false;
    }
  }

  return true;
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

interface LoginResult {
  passkey_id: number;
  activated: boolean;
  encryption_setup_finished: boolean;
}

/**
 * Start login and authenticate with passkey.
 * @param username Optional username. If provided and user exists, shows only that user's passkeys.
 *                 If not provided or user doesn't exist, shows all passkeys for this site.
 * @returns Login result with passkey_id and status flags for redirect decisions
 */
export async function login(username?: string): Promise<LoginResult> {
  // Check if passkeys are available on this device
  const passkeysAvailable = await checkPasskeysAvailable();
  if (!passkeysAvailable) {
    throw new Error("Passkeys are not available on this device");
  }

  // Get authentication options from server
  const optionsResponse = await fetch(`${API_PATH}/passkeys/login/start`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: username || null }),
  });

  if (!optionsResponse.ok) {
    const errorMsg = await getErrorMessage(
      optionsResponse,
      "Failed to start login",
    );
    throw new Error(errorMsg);
  }

  const response = await optionsResponse.json();
  const { session_id } = response;

  // Authenticate with passkey using browser API
  // webauthn-rs wraps options in { publicKey: ... }, simplewebauthn expects the inner object
  let credential: AuthenticationResponseJSON | null = null;
  try {
    credential = await startAuthentication({
      optionsJSON: response.publicKey,
    });
  } catch (error) {
    // User aborted or error occurred - clean up the challenge
    await deleteChallenge(session_id);
    throw error;
  }

  // Send credential to server to complete authentication (with retry for transient errors)
  const finishResponse = await fetchWithRetry(
    `${API_PATH}/passkeys/login/finish`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id, credential }),
    },
    { fallbackError: "Failed to complete login" },
  );

  const result = await finishResponse.json();

  // If user is not activated, redirect to claim page
  if (!result.activated) {
    window.location.href = `${LOGIN_PATH}/claim.html?reclaim=true`;
    // Return a promise that never resolves to prevent further processing
    return new Promise(() => {});
  }

  // Return the result so caller can determine redirect destination
  return result as LoginResult;
}

interface ServerConfig {
  no_signup: boolean;
  authenticated: boolean;
}

// Config is pre-fetched by the IIFE and available as a promise
declare const __CONFIG_PROMISE__: Promise<ServerConfig>;

function getConfig(): Promise<ServerConfig> {
  if (typeof __CONFIG_PROMISE__ !== "undefined") {
    return __CONFIG_PROMISE__;
  }
  // Fallback if IIFE config not available
  return Promise.resolve({ no_signup: false, authenticated: false });
}

// Wire up login events when DOM is ready
document.addEventListener("DOMContentLoaded", async () => {
  // Get server config (pre-fetched by IIFE, which also handles redirect if authenticated)
  const config = await getConfig();

  // Update register link with correct base path, show only if signups are enabled
  const registerLink = getOptionalElement("register-link");
  if (registerLink && !config.no_signup) {
    registerLink.setAttribute("href", `${LOGIN_PATH}/register.html`);
    registerLink.setAttribute("data-visible", "");
  }

  const usernameInput = getRequiredElement("username", HTMLInputElement);
  const loginButton = getRequiredElement("login-button", HTMLButtonElement);
  const passkeyButton = getOptionalElement("passkey-button", HTMLButtonElement);
  const errorMessage = getOptionalElement("error-message", HTMLDivElement);

  let isLoading = false;

  function showError(message: string): void {
    if (errorMessage) {
      errorMessage.textContent = message;
      errorMessage.hidden = false;
    }
  }

  function hideError(): void {
    if (errorMessage) {
      errorMessage.hidden = true;
      errorMessage.textContent = "";
    }
  }

  // Check if passkeys are supported before enabling the buttons
  const passkeysAvailable = await checkPasskeysAvailable();
  if (!passkeysAvailable) {
    loginButton.disabled = true;
    loginButton.textContent = "Passkeys not supported";
    if (passkeyButton) {
      passkeyButton.disabled = true;
      passkeyButton.textContent = "Passkeys not supported";
    }
    return;
  }

  loginButton.disabled = false;
  if (passkeyButton) {
    passkeyButton.disabled = false;
  }

  const handleLogin = async () => {
    if (isLoading) return;

    hideError();

    const username = usernameInput.value.trim();
    if (!username) {
      showError("Please enter your username");
      return;
    }

    isLoading = true;
    loginButton.disabled = true;
    if (passkeyButton) passkeyButton.disabled = true;
    try {
      const result = await login(username);

      // Redirect based on encryption setup status
      if (result.encryption_setup_finished) {
        window.location.href = APP_PATH;
      } else {
        window.location.href = `${APP_PATH}/setup-encryption.html`;
      }
    } catch (error) {
      console.error("Login failed:", error);
      // Don't show error for user abort (e.g., closing passkey dialog)
      if (error instanceof Error && error.name === "NotAllowedError") {
        // User cancelled - no need to show error
        return;
      }
      showError(error instanceof Error ? error.message : "Login failed");
    } finally {
      isLoading = false;
      loginButton.disabled = false;
      if (passkeyButton) passkeyButton.disabled = false;
    }
  };

  const handlePasskeyLogin = async () => {
    if (isLoading) return;

    hideError();

    isLoading = true;
    loginButton.disabled = true;
    if (passkeyButton) passkeyButton.disabled = true;
    try {
      // Login without username - browser shows all passkeys
      const result = await login();

      // Redirect based on encryption setup status
      if (result.encryption_setup_finished) {
        window.location.href = APP_PATH;
      } else {
        window.location.href = `${APP_PATH}/setup-encryption.html`;
      }
    } catch (error) {
      console.error("Passkey login failed:", error);
      // Don't show error for user abort (e.g., closing passkey dialog)
      if (error instanceof Error && error.name === "NotAllowedError") {
        // User cancelled - no need to show error
        return;
      }
      showError(error instanceof Error ? error.message : "Login failed");
    } finally {
      isLoading = false;
      loginButton.disabled = false;
      if (passkeyButton) passkeyButton.disabled = false;
    }
  };

  // Login button click (with username)
  loginButton.addEventListener("click", handleLogin);

  // Passkey button click (without username)
  passkeyButton?.addEventListener("click", handlePasskeyLogin);

  // Enter key in username input
  usernameInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      handleLogin();
    }
  });

  // Clear error when user starts typing
  usernameInput.addEventListener("input", hideError);
});
