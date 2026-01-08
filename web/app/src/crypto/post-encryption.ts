/**
 * Post encryption helpers.
 *
 * Handles encrypting and decrypting post content using the session key.
 */

import type { Post, PostSummary } from "../api/posts.ts";
import {
	decryptContent,
	ENCRYPTED_FORMAT_VERSION,
	encryptContent,
} from "./operations.ts";
import {
	getSessionEncryptionKey,
	isEncryptionEnabled,
	isUnlocked,
} from "./keystore.ts";

export interface EncryptedPostData {
	title: string;
	titleEncrypted: boolean;
	titleIv?: string;
	content: string;
	contentEncrypted: boolean;
	contentIv?: string;
	encryptionVersion?: number;
}

/**
 * Encrypt title and content if encryption is enabled.
 * Returns the encrypted data for sending to the API.
 */
export async function encryptPostData(
	title: string,
	content: string,
): Promise<EncryptedPostData> {
	if (!isEncryptionEnabled()) {
		return {
			title,
			titleEncrypted: false,
			content,
			contentEncrypted: false,
		};
	}

	const key = getSessionEncryptionKey();
	if (!key) {
		throw new Error("Encryption enabled but no key available");
	}

	const encryptedTitle = await encryptContent(title, key);
	const encryptedContent = await encryptContent(content, key);

	return {
		title: encryptedTitle.ciphertext,
		titleEncrypted: true,
		titleIv: encryptedTitle.iv,
		content: encryptedContent.ciphertext,
		contentEncrypted: true,
		contentIv: encryptedContent.iv,
		encryptionVersion: ENCRYPTED_FORMAT_VERSION,
	};
}

/**
 * Decrypt post content if encrypted and we have the key.
 * Returns the plaintext content, or a placeholder if decryption fails.
 */
export async function decryptPostContent(post: Post): Promise<string> {
	if (!post.content_encrypted) {
		return post.content;
	}

	if (!isUnlocked() || !post.iv || !post.encryption_version) {
		return post.content;
	}

	const key = getSessionEncryptionKey();
	if (!key) {
		return post.content;
	}

	try {
		return await decryptContent(
			post.content,
			post.iv,
			post.encryption_version,
			key,
		);
	} catch (err) {
		console.error("Failed to decrypt post:", err);
		return "[Decryption failed]";
	}
}

/**
 * Decrypt post title if encrypted and we have the key.
 * Returns the decrypted title, or the original title if decryption not possible.
 */
export async function decryptPostTitle(
	post: Post | PostSummary,
): Promise<string> {
	if (!post.title) {
		return "Untitled";
	}

	if (
		!post.title_encrypted ||
		!post.title_iv ||
		!("encryption_version" in post ? post.encryption_version : null)
	) {
		return post.title;
	}

	if (!isUnlocked()) {
		return post.title;
	}

	const key = getSessionEncryptionKey();
	if (!key) {
		return post.title;
	}

	const encryptionVersion =
		"encryption_version" in post ? post.encryption_version : null;
	if (!encryptionVersion) {
		return post.title;
	}

	try {
		return await decryptContent(
			post.title,
			post.title_iv,
			encryptionVersion,
			key,
		);
	} catch (err) {
		console.error("Failed to decrypt title:", err);
		return "[Decryption failed]";
	}
}

/**
 * Decrypt titles for a list of post summaries.
 * Returns a map of UUID -> decrypted title.
 * Does NOT modify the posts array.
 */
export async function decryptPostTitles(
	posts: PostSummary[],
): Promise<Map<string, string>> {
	const result = new Map<string, string>();

	for (const post of posts) {
		result.set(post.uuid, await decryptPostTitle(post));
	}

	return result;
}
