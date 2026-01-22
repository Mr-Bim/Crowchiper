/**
 * Posts API client.
 */

import * as v from "valibot";
import { fetchWithAuth } from "./auth.ts";
import { getErrorMessage } from "./utils.ts";
import {
  DeleteResponseSchema,
  PostNodeSchema,
  PostSchema,
  validate,
  type DeleteResponse,
  type Post,
  type PostNode,
} from "./schemas.ts";

declare const API_PATH: string;

// Re-export types for convenience
export type { DeleteResponse, Post, PostNode };

export interface CreatePostRequest {
  title?: string;
  title_encrypted?: boolean;
  title_iv?: string;
  content: string;
  content_encrypted?: boolean;
  iv?: string;
  encryption_version?: number;
  parent_id?: string;
}

export interface UpdatePostRequest {
  title?: string;
  title_encrypted?: boolean;
  title_iv?: string;
  content: string;
  content_encrypted?: boolean;
  iv?: string;
  encryption_version?: number;
  /** Optional attachment UUIDs to update refs (used with sendBeacon on page unload) */
  attachment_uuids?: string[];
}

export interface MovePostRequest {
  parent_id: string | null;
  position: number;
}

export interface ReorderRequest {
  parent_id: string | null;
  uuids: string[];
}

/**
 * Make an API request and validate the response.
 */
async function request<T>(
  path: string,
  schema: v.GenericSchema<T>,
  options: RequestInit = {},
): Promise<T> {
  const res = await fetchWithAuth(`${API_PATH}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!res.ok) {
    const errorMsg = await getErrorMessage(
      res,
      res.statusText || "Request failed",
    );
    throw new Error(errorMsg);
  }

  if (res.status === 204) {
    return undefined as T;
  }

  const data = await res.json();
  return validate(schema, data, path);
}

/**
 * Make an API request without response validation (for void responses).
 */
async function requestVoid(
  path: string,
  options: RequestInit = {},
): Promise<void> {
  const res = await fetchWithAuth(`${API_PATH}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!res.ok) {
    const errorMsg = await getErrorMessage(
      res,
      res.statusText || "Request failed",
    );
    throw new Error(errorMsg);
  }
}

/**
 * List posts as a tree structure.
 * @param depth - How many levels of children to include (default: 1)
 */
export async function listPosts(depth = 1): Promise<PostNode[]> {
  return request(`/posts?depth=${depth}`, v.array(PostNodeSchema));
}

/**
 * List children of a specific post (for lazy loading).
 */
export async function listPostChildren(uuid: string): Promise<PostNode[]> {
  return request(`/posts/${uuid}/children`, v.array(PostNodeSchema));
}

/**
 * Get a single post with full content.
 */
export async function getPost(uuid: string): Promise<Post> {
  return request(`/posts/${uuid}`, PostSchema);
}

/**
 * Create a new post.
 */
export async function createPost(data: CreatePostRequest): Promise<Post> {
  return request("/posts", PostSchema, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

/**
 * Update an existing post.
 */
export async function updatePost(
  uuid: string,
  data: UpdatePostRequest,
): Promise<Post> {
  return request(`/posts/${uuid}`, PostSchema, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

/**
 * Delete a post and all its children.
 */
export async function deletePost(uuid: string): Promise<DeleteResponse> {
  return request(`/posts/${uuid}`, DeleteResponseSchema, {
    method: "DELETE",
  });
}

/**
 * Reorder posts within a parent.
 * @param parentId - Parent post UUID, or null for root level
 * @param uuids - New order of child UUIDs
 */
export async function reorderPosts(
  parentId: string | null,
  uuids: string[],
): Promise<void> {
  return requestVoid("/posts/reorder", {
    method: "POST",
    body: JSON.stringify({ parent_id: parentId, uuids }),
  });
}

/**
 * Move a post to a new parent at a specific position.
 */
export async function movePost(
  uuid: string,
  parentId: string | null,
  position: number,
): Promise<void> {
  return requestVoid(`/posts/${uuid}/move`, {
    method: "POST",
    body: JSON.stringify({ parent_id: parentId, position }),
  });
}

/**
 * Update a post using sendBeacon (for page unload).
 * Returns true if the beacon was queued successfully.
 */
export function updatePostBeacon(
  uuid: string,
  data: UpdatePostRequest,
): boolean {
  return navigator.sendBeacon(
    `${API_PATH}/posts/${uuid}`,
    new Blob([JSON.stringify(data)], { type: "application/json" }),
  );
}
