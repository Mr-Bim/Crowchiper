/**
 * State management for posts and editor.
 *
 * Centralizes all mutable state for the app.
 */

import type { Post, PostSummary } from "../api/posts.ts";
import type { EditorView } from "../editor/setup.ts";

let editor: EditorView | null = null;
let posts: PostSummary[] = [];
let currentPost: Post | null = null;
let currentDecryptedContent: string | null = null;
let currentDecryptedTitle: string | null = null;
let decryptedTitles: Map<string, string> = new Map();
let saveTimeout: number | null = null;
let previousAttachmentUuids: string[] = [];

// Pending encrypted data (encrypted locally, not yet saved to server)
export interface PendingEncryptedData {
	title: string;
	titleEncrypted: boolean;
	titleIv: string | null;
	content: string;
	contentEncrypted: boolean;
	contentIv: string | null;
	encryptionVersion: number | null;
}

let pendingEncryptedData: PendingEncryptedData | null = null;

// Whether there are unsaved changes since last server sync
let isDirty = false;

// Timer for periodic server save
let serverSaveInterval: number | null = null;

// --- Editor ---

export function getEditor(): EditorView | null {
	return editor;
}

export function setEditor(e: EditorView | null): void {
	editor = e;
}

// --- Posts ---

export function getPosts(): PostSummary[] {
	return posts;
}

export function setPosts(p: PostSummary[]): void {
	posts = p;
}

export function addPost(post: PostSummary): void {
	posts.unshift(post);
}

export function removePost(uuid: string): void {
	posts = posts.filter((p) => p.uuid !== uuid);
}

export function updatePostInList(
	uuid: string,
	updates: Partial<PostSummary>,
): void {
	const idx = posts.findIndex((p) => p.uuid === uuid);
	if (idx !== -1) {
		posts[idx] = { ...posts[idx], ...updates };
	}
}

export function sortPostsByPosition(): void {
	// Posts are returned from server already sorted by position
	// This is a no-op but kept for clarity
}

export function movePost(fromIndex: number, toIndex: number): void {
	if (fromIndex === toIndex) return;
	if (fromIndex < 0 || fromIndex >= posts.length) return;
	if (toIndex < 0 || toIndex >= posts.length) return;

	const [removed] = posts.splice(fromIndex, 1);
	posts.splice(toIndex, 0, removed);
}

export function getPostUuids(): string[] {
	return posts.map((p) => p.uuid);
}

// --- Current Post ---

export function getCurrentPost(): Post | null {
	return currentPost;
}

export function setCurrentPost(post: Post | null): void {
	currentPost = post;
}

export function getCurrentDecryptedContent(): string | null {
	return currentDecryptedContent;
}

export function setCurrentDecryptedContent(content: string | null): void {
	currentDecryptedContent = content;
}

// --- Current Decrypted Title ---

export function getCurrentDecryptedTitle(): string | null {
	return currentDecryptedTitle;
}

export function setCurrentDecryptedTitle(title: string | null): void {
	currentDecryptedTitle = title;
}

// --- Decrypted Titles Map (for post list display) ---

export function getDecryptedTitles(): Map<string, string> {
	return decryptedTitles;
}

export function setDecryptedTitles(titles: Map<string, string>): void {
	decryptedTitles = titles;
}

export function setDecryptedTitle(uuid: string, title: string): void {
	decryptedTitles.set(uuid, title);
}

// --- Save Timeout ---

export function getSaveTimeout(): number | null {
	return saveTimeout;
}

export function setSaveTimeout(timeout: number | null): void {
	saveTimeout = timeout;
}

export function clearSaveTimeout(): void {
	if (saveTimeout) {
		clearTimeout(saveTimeout);
		saveTimeout = null;
	}
}

// --- Previous Attachment UUIDs ---

export function getPreviousAttachmentUuids(): string[] {
	return previousAttachmentUuids;
}

export function setPreviousAttachmentUuids(uuids: string[]): void {
	previousAttachmentUuids = uuids;
}

// --- Pending Encrypted Data ---

export function getPendingEncryptedData(): PendingEncryptedData | null {
	return pendingEncryptedData;
}

export function setPendingEncryptedData(data: PendingEncryptedData | null): void {
	pendingEncryptedData = data;
}

// --- Dirty Flag ---

export function getIsDirty(): boolean {
	return isDirty;
}

export function setIsDirty(dirty: boolean): void {
	isDirty = dirty;
}

// --- Server Save Interval ---

export function getServerSaveInterval(): number | null {
	return serverSaveInterval;
}

export function setServerSaveInterval(interval: number | null): void {
	serverSaveInterval = interval;
}

export function clearServerSaveInterval(): void {
	if (serverSaveInterval) {
		clearInterval(serverSaveInterval);
		serverSaveInterval = null;
	}
}
