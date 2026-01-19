import {
  type AuthenticationResponseJSON,
  startAuthentication,
  startRegistration,
} from "@simplewebauthn/browser";
import { getErrorMessage } from "./api-utils.ts";

declare const API_PATH: string;
declare const LOGIN_PATH: string;
declare const APP_PATH: string;

type ClaimMode = "register" | "reclaim";

interface ServerConfig {
  no_signup: boolean;
  authenticated: boolean;
}

async function fetchConfig(): Promise<ServerConfig> {
  try {
    const response = await fetch(`${API_PATH}/config`);
    if (response.ok) {
      return await response.json();
    }
  } catch {
    // Ignore errors
  }
  return { no_signup: false, authenticated: false };
}

async function startPasskeyRegistration(uuid: string): Promise<void> {
  const optionsResponse = await fetch(`${API_PATH}/passkeys/register/start`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ uuid }),
  });

  if (!optionsResponse.ok) {
    const errorMsg = await getErrorMessage(
      optionsResponse,
      "Failed to start registration",
    );
    throw new Error(errorMsg);
  }

  const options = await optionsResponse.json();

  const credential = await startRegistration({
    optionsJSON: options.publicKey,
  });

  const finishResponse = await fetch(`${API_PATH}/passkeys/register/finish`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ uuid, credential }),
  });

  if (!finishResponse.ok) {
    const errorMsg = await getErrorMessage(
      finishResponse,
      "Failed to complete registration",
    );
    throw new Error(errorMsg);
  }
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

async function reclaimAccount(): Promise<void> {
  // Start claim flow - this uses discoverable authentication
  const optionsResponse = await fetch(`${API_PATH}/passkeys/claim/start`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  });

  if (!optionsResponse.ok) {
    const errorMsg = await getErrorMessage(
      optionsResponse,
      "Failed to start claim",
    );
    throw new Error(errorMsg);
  }

  const response = await optionsResponse.json();
  const { session_id } = response;

  // Authenticate with passkey
  let credential: AuthenticationResponseJSON | null = null;
  try {
    credential = await startAuthentication({
      optionsJSON: response.publicKey,
    });
  } catch (error) {
    await deleteChallenge(session_id);
    throw error;
  }

  // Finish claim - this activates the user and sets the auth cookie
  const finishResponse = await fetch(`${API_PATH}/passkeys/claim/finish`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ session_id, credential }),
  });

  if (!finishResponse.ok) {
    const errorMsg = await getErrorMessage(
      finishResponse,
      "Failed to complete claim",
    );
    throw new Error(errorMsg);
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  const titleEl = document.getElementById("claim-title") as HTMLHeadingElement;
  const statusEl = document.getElementById("status") as HTMLParagraphElement;
  const claimButton = document.getElementById(
    "claim-button",
  ) as HTMLButtonElement;

  const params = new URLSearchParams(window.location.search);
  const uuid = params.get("uuid");
  const reclaim = params.get("reclaim") === "true";

  // For reclaim mode (no uuid), check if already authenticated
  if (!uuid) {
    const config = await fetchConfig();
    if (config.authenticated) {
      window.location.href = APP_PATH;
      return;
    }
  }

  // Determine mode: if uuid is provided, it's a new registration
  // If reclaim=true, it's a reclaim flow
  let mode: ClaimMode;
  if (uuid) {
    mode = "register";
    titleEl.textContent = "Claim Admin";
    claimButton.textContent = "Register Passkey";
    statusEl.textContent = "Click the button to register your passkey";
  } else if (reclaim) {
    mode = "reclaim";
    titleEl.textContent = "Reclaim Account";
    claimButton.textContent = "Use Passkey";
    statusEl.textContent =
      "Your account needs to be reclaimed. Use your passkey to activate it.";
  } else {
    statusEl.textContent = "Invalid claim link";
    return;
  }

  claimButton.disabled = false;

  let isLoading = false;

  claimButton.addEventListener("click", async () => {
    if (isLoading) return;

    isLoading = true;
    claimButton.disabled = true;

    try {
      if (mode === "register") {
        if (uuid == null) {
          window.location.href = `${LOGIN_PATH}/index.html`;
          return;
        }
        statusEl.textContent = "Registering passkey...";
        await startPasskeyRegistration(uuid);
        statusEl.textContent = "Registration successful! Redirecting...";
        window.location.href = `${LOGIN_PATH}/index.html`;
      } else {
        statusEl.textContent = "Authenticating...";
        await reclaimAccount();
        statusEl.textContent = "Account reclaimed! Redirecting...";
        window.location.href = APP_PATH;
      }
    } catch (error) {
      console.error("Claim failed:", error);
      // Don't show error for user abort
      if (error instanceof Error && error.name === "NotAllowedError") {
        statusEl.textContent =
          mode === "register"
            ? "Click the button to register your passkey"
            : "Your account needs to be reclaimed. Use your passkey to activate it.";
      } else {
        statusEl.textContent =
          error instanceof Error ? error.message : "Claim failed";
      }
      isLoading = false;
      claimButton.disabled = false;
    }
  });
});
