/**
 * Shared API utilities for both login (public) and app pages.
 */

/**
 * Safely extract an error message from a fetch Response.
 *
 * Handles cases where the server returns non-JSON (e.g., Cloudflare 500 HTML pages).
 * Returns a user-friendly error message with optional status code for debugging.
 */
export async function getErrorMessage(
  response: Response,
  fallback: string,
  includeStatus = true,
): Promise<string> {
  const status = response.status;
  try {
    const text = await response.text();
    if (!text) {
      return includeStatus ? `${fallback} (${status})` : fallback;
    }

    // Try to parse as JSON
    const json = JSON.parse(text);
    if (json.error && typeof json.error === "string") {
      return json.error;
    }

    // JSON but no error field - include raw text (truncated)
    const preview = text.length > 100 ? text.slice(0, 100) + "..." : text;
    return includeStatus ? `${fallback} (${status}): ${preview}` : fallback;
  } catch {
    // JSON parse failed - likely HTML error page
    return includeStatus ? `${fallback} (${status})` : fallback;
  }
}

/**
 * Check if an error is likely a transient server error that could be retried.
 */
function isRetryableStatus(status: number): boolean {
  return status >= 500 && status < 600;
}

export interface RetryOptions {
  maxRetries?: number;
  delayMs?: number;
  fallbackError: string;
}

/**
 * Fetch with automatic retry for transient server errors (5xx).
 *
 * Retries the request up to maxRetries times if the server returns a 5xx error.
 * This is useful for handling transient issues like Cloudflare errors.
 *
 * @param fetchFn - The fetch function to use (allows injecting fetchWithAuth)
 */
export async function fetchWithRetry(
  input: RequestInfo | URL,
  init: RequestInit,
  options: RetryOptions,
  fetchFn: typeof fetch = fetch,
): Promise<Response> {
  const { maxRetries = 2, delayMs = 1000, fallbackError } = options;

  const attempt = async (retriesLeft: number): Promise<Response> => {
    const response = await fetchFn(input, init);

    if (response.ok) {
      return response;
    }

    if (isRetryableStatus(response.status) && retriesLeft > 0) {
      const errorMsg = await getErrorMessage(response, "Server error");
      console.warn(
        `Request failed with ${response.status}: ${errorMsg}, retrying...`,
      );
      await new Promise((resolve) => setTimeout(resolve, delayMs));
      return attempt(retriesLeft - 1);
    }

    const errorMsg = await getErrorMessage(response, fallbackError);
    throw new Error(errorMsg);
  };

  return attempt(maxRetries);
}
