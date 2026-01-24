/**
 * Authentication utilities for API requests.
 *
 * The server handles token refresh automatically - if the access token is expired
 * but the refresh token is valid, the server issues a new access token cookie.
 *
 * If we get a 401, it means both tokens are invalid/expired, so redirect to login.
 */

declare const LOGIN_PATH: string;

// Default request timeout in milliseconds (30 seconds)
const DEFAULT_TIMEOUT_MS = 30000;

/**
 * Wrapper for fetch that handles 401 responses by redirecting to login.
 * The server handles token refresh automatically via cookies.
 * Includes a default 30-second timeout to prevent indefinite hangs.
 *
 * @param input - URL or Request object
 * @param init - RequestInit options (extended with optional timeoutMs)
 */
export async function fetchWithAuth(
  input: RequestInfo | URL,
  init?: RequestInit & { timeoutMs?: number },
): Promise<Response> {
  const { timeoutMs = DEFAULT_TIMEOUT_MS, ...fetchInit } = init ?? {};

  // Create abort controller for timeout
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(input, {
      ...fetchInit,
      credentials: "include",
      signal: controller.signal,
    });

    if (response.status === 401) {
      // Both access and refresh tokens are invalid - redirect to login
      window.location.href = LOGIN_PATH;
      // Return response in case code continues (it won't due to redirect)
      return response;
    }

    return response;
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") {
      throw new Error(`Request timed out after ${timeoutMs}ms`);
    }
    throw error;
  } finally {
    clearTimeout(timeoutId);
  }
}
