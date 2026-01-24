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

/** Error thrown when an attachment is not found (404) */
export class AttachmentNotFoundError extends Error {
  constructor(uuid: string) {
    super(`Attachment not found: ${uuid}`);
    this.name = "AttachmentNotFoundError";
  }
}

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

/** Error thrown when upload is aborted */
export class UploadAbortedError extends Error {
  constructor() {
    super("Upload aborted");
    this.name = "UploadAbortedError";
  }
}

/** Options for upload with progress */
export interface UploadWithProgressOptions {
  onProgress: UploadProgressCallback;
  signal?: AbortSignal;
}

/**
 * Upload an encrypted attachment with progress tracking.
 * Uses XMLHttpRequest for upload progress events.
 * Supports abort via AbortSignal.
 */
export function uploadAttachmentWithProgress(
  req: UploadAttachmentRequest,
  options: UploadWithProgressOptions,
): Promise<UploadAttachmentResponse> {
  const { onProgress, signal } = options;

  return new Promise((resolve, reject) => {
    // Check if already aborted
    if (signal?.aborted) {
      reject(new UploadAbortedError());
      return;
    }

    const xhr = new XMLHttpRequest();
    const formData = buildUploadFormData(req);

    // Handle abort signal
    const abortHandler = () => {
      xhr.abort();
      reject(new UploadAbortedError());
    };
    signal?.addEventListener("abort", abortHandler);

    const cleanup = () => {
      signal?.removeEventListener("abort", abortHandler);
    };

    xhr.upload.addEventListener("progress", (e) => {
      if (e.lengthComputable) {
        const percent = Math.round((e.loaded / e.total) * 100);
        onProgress(percent);
      }
    });

    xhr.addEventListener("load", () => {
      cleanup();
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
      cleanup();
      reject(new Error("Network error during upload"));
    });

    xhr.addEventListener("timeout", () => {
      cleanup();
      reject(new Error("Upload timed out"));
    });

    xhr.addEventListener("abort", () => {
      cleanup();
      // Don't reject here - abortHandler already did
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
 * Throws AttachmentNotFoundError if the attachment doesn't exist.
 */
export async function getAttachmentThumbnail(
  uuid: string,
  size: ThumbnailSize,
): Promise<BinaryAttachmentResponse> {
  const response = await fetchWithAuth(
    `${API_PATH}/attachments/${uuid}/thumbnail/${size}`,
  );

  if (!response.ok) {
    if (response.status === 404) {
      throw new AttachmentNotFoundError(uuid);
    }
    const errorMsg = await getErrorMessage(response, "Failed to get thumbnail");
    throw new Error(errorMsg);
  }

  const iv = response.headers.get("X-Encryption-IV") ?? "";

  const data = await response.arrayBuffer();
  return { data, iv };
}
