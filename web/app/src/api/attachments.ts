/**
 * Attachments API client for encrypted image uploads.
 * Uses binary streaming instead of base64 for efficiency.
 */

import { fetchWithAuth } from "./auth.ts";
import { getErrorMessage } from "./utils.ts";
import {
  UploadAttachmentResponseSchema,
  validate,
  type UploadAttachmentResponse,
} from "./schemas.ts";

declare const API_PATH: string;

// Re-export types for convenience
export type { UploadAttachmentResponse };

export interface UploadAttachmentRequest {
  image: ArrayBuffer;
  image_iv: string;
  thumb_sm: ArrayBuffer;
  thumb_sm_iv: string;
  thumb_md: ArrayBuffer;
  thumb_md_iv: string;
  thumb_lg: ArrayBuffer;
  thumb_lg_iv: string;
  encryption_version: number;
}

export interface BinaryAttachmentResponse {
  data: ArrayBuffer;
  iv: string;
}

export type ThumbnailSize = "sm" | "md" | "lg";

/**
 * Upload an encrypted attachment using multipart form data.
 */
export async function uploadAttachment(
  req: UploadAttachmentRequest,
): Promise<UploadAttachmentResponse> {
  const formData = new FormData();
  formData.append("image", new Blob([req.image]));
  formData.append("image_iv", req.image_iv);
  formData.append("thumb_sm", new Blob([req.thumb_sm]));
  formData.append("thumb_sm_iv", req.thumb_sm_iv);
  formData.append("thumb_md", new Blob([req.thumb_md]));
  formData.append("thumb_md_iv", req.thumb_md_iv);
  formData.append("thumb_lg", new Blob([req.thumb_lg]));
  formData.append("thumb_lg_iv", req.thumb_lg_iv);
  formData.append("encryption_version", req.encryption_version.toString());

  const response = await fetchWithAuth(`${API_PATH}/attachments`, {
    method: "POST",
    body: formData,
  });

  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to upload attachment",
    );
    throw new Error(errorMsg);
  }

  const data = await response.json();
  return validate(UploadAttachmentResponseSchema, data, "upload response");
}

/**
 * Get an encrypted attachment by UUID as binary data.
 * Returns the encrypted data and IV from header.
 */
export async function getAttachment(
  uuid: string,
): Promise<BinaryAttachmentResponse> {
  const response = await fetchWithAuth(`${API_PATH}/attachments/${uuid}`);

  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to get attachment",
    );
    throw new Error(errorMsg);
  }

  const iv = response.headers.get("X-Encryption-IV");
  if (!iv) {
    throw new Error("Missing X-Encryption-IV header");
  }

  const data = await response.arrayBuffer();
  return { data, iv };
}

/**
 * Get a single encrypted thumbnail by attachment UUID and size.
 * Returns the encrypted data and IV from header.
 */
export async function getAttachmentThumbnail(
  uuid: string,
  size: ThumbnailSize,
): Promise<BinaryAttachmentResponse> {
  const response = await fetchWithAuth(
    `${API_PATH}/attachments/${uuid}/thumbnail/${size}`,
  );

  if (!response.ok) {
    const errorMsg = await getErrorMessage(response, "Failed to get thumbnail");
    throw new Error(errorMsg);
  }

  const iv = response.headers.get("X-Encryption-IV");
  if (!iv) {
    throw new Error("Missing X-Encryption-IV header");
  }

  const data = await response.arrayBuffer();
  return { data, iv };
}
