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
	created_at: string;
	updated_at: string;
}

export interface PostSummary {
	uuid: string;
	title: string | null;
	title_encrypted: boolean;
	title_iv: string | null;
	content_encrypted: boolean;
	encryption_version: number | null;
	position: number | null;
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

export async function listPosts(): Promise<PostSummary[]> {
	return request<PostSummary[]>("/posts");
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

export async function deletePost(uuid: string): Promise<void> {
	return request<void>(`/posts/${uuid}`, {
		method: "DELETE",
	});
}

export async function reorderPosts(uuids: string[]): Promise<void> {
	return request<void>("/posts/reorder", {
		method: "POST",
		body: JSON.stringify({ uuids }),
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
