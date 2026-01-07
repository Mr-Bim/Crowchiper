import { startRegistration } from "@simplewebauthn/browser";
import { getErrorMessage } from "./api-utils.ts";

declare const API_PATH: string;
declare const LOGIN_PATH: string;
declare const APP_PATH: string;

interface ServerConfig {
	no_signup: boolean;
}

async function fetchConfig(): Promise<ServerConfig> {
	try {
		const response = await fetch(`${API_PATH}/config`);
		if (response.ok) {
			return await response.json();
		}
	} catch {
		// Ignore errors - default to allowing signup
	}
	return { no_signup: false };
}

interface ClaimedUser {
	uuid: string;
	username: string;
}

let claimedUser: ClaimedUser | null = null;

async function claimUsername(username: string): Promise<ClaimedUser> {
	const response = await fetch(`${API_PATH}/users`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ username }),
	});

	if (!response.ok) {
		const errorMsg = await getErrorMessage(
			response,
			"Failed to claim username",
		);
		throw new Error(errorMsg);
	}

	return response.json();
}

async function freeUsername(uuid: string): Promise<void> {
	await fetch(`${API_PATH}/users/${uuid}`, {
		method: "DELETE",
	});
}

async function freeClaimed(): Promise<void> {
	if (claimedUser) {
		await freeUsername(claimedUser.uuid);
		claimedUser = null;
	}
}

async function startPasskeyRegistration(
	uuid: string,
	authenticatorType: string,
): Promise<void> {
	// Get registration options from server
	const optionsResponse = await fetch(`${API_PATH}/passkeys/register/start`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ uuid, authenticator_type: authenticatorType }),
	});

	if (!optionsResponse.ok) {
		const errorMsg = await getErrorMessage(
			optionsResponse,
			"Failed to start registration",
		);
		throw new Error(errorMsg);
	}

	const options = await optionsResponse.json();

	// Create passkey using browser API
	// webauthn-rs wraps options in { publicKey: ... }, simplewebauthn expects the inner object
	const credential = await startRegistration({
		optionsJSON: options.publicKey,
	});

	// Send credential to server to complete registration
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

export async function register(
	username: string,
	authenticatorType: string,
): Promise<void> {
	// Free any previously claimed username
	await freeClaimed();

	// Claim the new username
	claimedUser = await claimUsername(username);

	try {
		// Start passkey registration
		await startPasskeyRegistration(claimedUser.uuid, authenticatorType);

		// Registration successful - user is now activated and logged in
		claimedUser = null; // Don't free on page unload

		// Redirect to encryption setup (new users always need this)
		window.location.href = `${APP_PATH}/setup-encryption.html`;
	} catch (error) {
		// If passkey registration fails, free the claimed username
		await freeClaimed();
		throw error;
	}
}

function isAndroid(): boolean {
	return /Android/i.test(navigator.userAgent);
}

// Wire up register events when DOM is ready
document.addEventListener("DOMContentLoaded", async () => {
	// Check if signups are disabled, redirect to login if so
	const config = await fetchConfig();
	if (config.no_signup) {
		window.location.href = `${LOGIN_PATH}/index.html`;
		return;
	}

	// Update login link with correct base path
	const loginLink = document.getElementById("login-link");
	if (loginLink) {
		loginLink.setAttribute("href", `${LOGIN_PATH}/index.html`);
	}

	// Show authenticator type selection only on Android
	const authTypeFieldset = document.getElementById("auth-type-fieldset");
	if (authTypeFieldset && isAndroid()) {
		authTypeFieldset.hidden = false;
	}

	const usernameInput = document.getElementById("username") as HTMLInputElement;
	const registerButton = document.getElementById(
		"register-button",
	) as HTMLButtonElement;
	const errorMessage = document.getElementById("error-message");

	let isLoading = false;
	registerButton.disabled = false;

	function showError(message: string): void {
		if (errorMessage) {
			errorMessage.textContent = message;
			errorMessage.hidden = false;
		}
	}

	function hideError(): void {
		if (errorMessage) {
			errorMessage.hidden = true;
		}
	}

	const handleRegister = async () => {
		if (isLoading) return;

		const username = usernameInput?.value.trim();
		if (username) {
			hideError();
			isLoading = true;
			registerButton.disabled = true;
			try {
				// On Android, use the selected authenticator type; elsewhere default to security_key
				let authenticatorType = "security_key";
				if (isAndroid()) {
					const authTypeInput = document.querySelector(
						'input[name="auth-type"]:checked',
					) as HTMLInputElement;
					authenticatorType = authTypeInput?.value || "security_key";
				}
				await register(username, authenticatorType);
			} catch (error) {
				// Ignore user abort
				if (error instanceof Error && error.name === "NotAllowedError") {
					return;
				}
				console.error("Registration failed:", error);
				showError(
					error instanceof Error ? error.message : "Registration failed",
				);
			} finally {
				isLoading = false;
				registerButton.disabled = false;
			}
		}
	};

	// Register button click
	registerButton?.addEventListener("click", handleRegister);

	// Enter key in username input
	usernameInput?.addEventListener("keydown", (e) => {
		if (e.key === "Enter") {
			handleRegister();
		}
	});

	// Hide error when user starts typing
	usernameInput?.addEventListener("input", hideError);

	// Free claimed username when leaving the page
	window.addEventListener("beforeunload", () => {
		if (claimedUser) {
			// Use fetch with keepalive for reliable delivery during page unload
			fetch(`${API_PATH}/users/${claimedUser.uuid}`, {
				method: "DELETE",
				keepalive: true,
			});
		}
	});
});
