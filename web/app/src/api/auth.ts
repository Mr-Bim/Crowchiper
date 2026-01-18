/**
 * Authentication utilities for API requests.
 *
 * The server handles token refresh automatically - if the access token is expired
 * but the refresh token is valid, the server issues a new access token cookie.
 *
 * If we get a 401, it means both tokens are invalid/expired, so redirect to login.
 */

declare const LOGIN_PATH: string;

/**
 * Wrapper for fetch that handles 401 responses by redirecting to login.
 * The server handles token refresh automatically via cookies.
 */
export async function fetchWithAuth(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const response = await fetch(input, {
    ...init,
    credentials: "include",
  });

  if (response.status === 401) {
    // Both access and refresh tokens are invalid - redirect to login
    window.location.href = LOGIN_PATH;
    // Return response in case code continues (it won't due to redirect)
    return response;
  }

  return response;
}
