/**
 * API utilities for the authenticated app.
 *
 * Re-exports shared utilities and provides auth-aware versions.
 */

import { fetchWithAuth } from "./auth.ts";
import {
  getErrorMessage,
  fetchWithRetry as baseFetchWithRetry,
  type RetryOptions,
} from "../../../shared/api-utils.ts";

// Re-export getErrorMessage as-is
export { getErrorMessage };

/**
 * Fetch with automatic retry for transient server errors (5xx).
 *
 * Uses fetchWithAuth for authentication handling (401 redirects to login).
 * Retries the request up to maxRetries times if the server returns a 5xx error.
 */
export async function fetchWithRetry(
  input: RequestInfo | URL,
  init: RequestInit,
  options: RetryOptions,
): Promise<Response> {
  return baseFetchWithRetry(input, init, options, fetchWithAuth);
}
