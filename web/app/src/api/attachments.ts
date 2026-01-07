/**
 * Attachments API client for encrypted image uploads.
 */

import { getErrorMessage } from "./utils.ts";

declare const API_PATH: string;

export interface UploadAttachmentRequest {
	encrypted_image: string; // base64url
	encrypted_image_iv: string;
	encrypted_thumbnail: string; // base64url
	encrypted_thumbnail_iv: string;
	encryption_version: number;
}

export interface UploadAttachmentResponse {
	uuid: string;
}

export interface AttachmentResponse {
	encrypted_image: string; // base64url
	iv: string;
}

export interface ThumbnailResponse {
	encrypted_thumbnail: string; // base64url
	iv: string;
}

/**
 * Upload an encrypted attachment.
 */
export async function uploadAttachment(
	req: UploadAttachmentRequest,
): Promise<UploadAttachmentResponse> {
	const response = await fetch(`${API_PATH}/attachments`, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
		},
		credentials: "include",
		body: JSON.stringify(req),
	});

	if (!response.ok) {
		const errorMsg = await getErrorMessage(response, "Failed to upload attachment");
		throw new Error(errorMsg);
	}

	return response.json();
}

/**
 * Get an encrypted attachment by UUID.
 */
export async function getAttachment(uuid: string): Promise<AttachmentResponse> {
	const response = await fetch(`${API_PATH}/attachments/${uuid}`, {
		credentials: "include",
	});

	if (!response.ok) {
		const errorMsg = await getErrorMessage(response, "Failed to get attachment");
		throw new Error(errorMsg);
	}

	return response.json();
}

/**
 * Get an encrypted thumbnail by attachment UUID.
 */
export async function getAttachmentThumbnail(
	uuid: string,
): Promise<ThumbnailResponse> {
	const response = await fetch(`${API_PATH}/attachments/${uuid}/thumbnail`, {
		credentials: "include",
	});

	if (!response.ok) {
		const errorMsg = await getErrorMessage(response, "Failed to get thumbnail");
		throw new Error(errorMsg);
	}

	return response.json();
}
