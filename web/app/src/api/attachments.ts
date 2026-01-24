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
declare const LOGIN_PATH: string;

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

/** Progress callback for upload tracking */
export type UploadProgressCallback = (percent: number) => void;

/**
 * Build FormData for attachment upload.
 */
function buildUploadFormData(req: UploadAttachmentRequest): FormData {
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
  return formData;
}

/**
 * Upload an encrypted attachment using multipart form data.
 */
export async function uploadAttachment(
  req: UploadAttachmentRequest,
): Promise<UploadAttachmentResponse> {
  const formData = buildUploadFormData(req);

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
 * Upload an encrypted attachment with progress tracking.
 * Uses XMLHttpRequest for upload progress events.
 */
export function uploadAttachmentWithProgress(
  req: UploadAttachmentRequest,
  onProgress: UploadProgressCallback,
): Promise<UploadAttachmentResponse> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const formData = buildUploadFormData(req);

    xhr.upload.addEventListener("progress", (e) => {
      if (e.lengthComputable) {
        const percent = Math.round((e.loaded / e.total) * 100);
        onProgress(percent);
      }
    });

    xhr.addEventListener("load", () => {
      if (xhr.status === 401) {
        window.location.href = LOGIN_PATH;
        reject(new Error("Authentication required"));
        return;
      }

      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const data = JSON.parse(xhr.responseText);
          const result = validate(
            UploadAttachmentResponseSchema,
            data,
            "upload response",
          );
          resolve(result);
        } catch (err) {
          reject(err);
        }
      } else {
        reject(new Error(`Upload failed with status ${xhr.status}`));
      }
    });

    xhr.addEventListener("error", () => {
      reject(new Error("Network error during upload"));
    });

    xhr.addEventListener("timeout", () => {
      reject(new Error("Upload timed out"));
    });

    xhr.open("POST", `${API_PATH}/attachments`);
    xhr.withCredentials = true;
    xhr.timeout = 120000; // 2 minute timeout for large uploads
    xhr.send(formData);
  });
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

  const iv = response.headers.get("X-Encryption-IV") ?? "";

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

  const iv = response.headers.get("X-Encryption-IV") ?? "";

  const data = await response.arrayBuffer();
  return { data, iv };
}
