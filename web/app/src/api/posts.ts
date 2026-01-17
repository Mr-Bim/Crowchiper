import { getErrorMessage } from "./utils.ts";

declare const API_PATH: string;

export interface Post {
  uuid: string;
  title: string | null;
  title_encrypted: boolean;
  title_iv: string | null;
  content: string;
  content_encrypted: boolean;
  iv: string | null;
  encryption_version: number | null;
  position: number | null;
  parent_id: string | null;
  is_folder: boolean;
  created_at: string;
  updated_at: string;
}

export interface PostNode {
  uuid: string;
  title: string | null;
  title_encrypted: boolean;
  title_iv: string | null;
  content_encrypted: boolean;
  encryption_version: number | null;
  position: number | null;
  parent_id: string | null;
  is_folder: boolean;
  has_children: boolean;
  children: PostNode[] | null;
  created_at: string;
  updated_at: string;
}

export interface CreatePostRequest {
  title?: string;
  title_encrypted?: boolean;
  title_iv?: string;
  content: string;
  content_encrypted?: boolean;
  iv?: string;
  encryption_version?: number;
  parent_id?: string;
  is_folder?: boolean;
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

export interface DeleteResponse {
  deleted: boolean;
  children_deleted: number;
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(`${API_PATH}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
    credentials: "include",
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

  return res.json();
}

export async function listPosts(depth = 1): Promise<PostNode[]> {
  return request<PostNode[]>(`/posts?depth=${depth}`);
}

export async function listPostChildren(uuid: string): Promise<PostNode[]> {
  return request<PostNode[]>(`/posts/${uuid}/children`);
}

export async function getPost(uuid: string): Promise<Post> {
  return request<Post>(`/posts/${uuid}`);
}

export async function createPost(data: CreatePostRequest): Promise<Post> {
  return request<Post>("/posts", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updatePost(
  uuid: string,
  data: UpdatePostRequest,
): Promise<Post> {
  return request<Post>(`/posts/${uuid}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deletePost(uuid: string): Promise<DeleteResponse> {
  return request<DeleteResponse>(`/posts/${uuid}`, {
    method: "DELETE",
  });
}

export async function reorderPosts(
  parentId: string | null,
  uuids: string[],
): Promise<void> {
  return request<void>("/posts/reorder", {
    method: "POST",
    body: JSON.stringify({ parent_id: parentId, uuids }),
  });
}

export async function movePost(
  uuid: string,
  parentId: string | null,
  position: number,
): Promise<void> {
  return request<void>(`/posts/${uuid}/move`, {
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
