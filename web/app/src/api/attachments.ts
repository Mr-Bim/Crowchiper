/**
 * Attachments API client for encrypted image uploads.
 * Uses binary streaming instead of base64 for efficiency.
 */

import { getErrorMessage } from "./utils.ts";

declare const API_PATH: string;

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

export interface UploadAttachmentResponse {
  uuid: string;
}

export interface BinaryAttachmentResponse {
  data: ArrayBuffer;
  iv: string;
}

export interface ThumbnailData {
  data: ArrayBuffer;
  iv: string;
}

export interface ThumbnailsResponse {
  sm: ThumbnailData;
  md?: ThumbnailData;
  lg?: ThumbnailData;
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

  const response = await fetch(`${API_PATH}/attachments`, {
    method: "POST",
    credentials: "include",
    body: formData,
  });

  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to upload attachment",
    );
    throw new Error(errorMsg);
  }

  return response.json();
}

/**
 * Get an encrypted attachment by UUID as binary data.
 * Returns the encrypted data and IV from header.
 */
export async function getAttachment(
  uuid: string,
): Promise<BinaryAttachmentResponse> {
  const response = await fetch(`${API_PATH}/attachments/${uuid}`, {
    credentials: "include",
  });

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
  const response = await fetch(
    `${API_PATH}/attachments/${uuid}/thumbnail/${size}`,
    {
      credentials: "include",
    },
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

/**
 * Find a byte sequence in a Uint8Array.
 * Returns the index of the first occurrence, or -1 if not found.
 */
function findBytes(
  haystack: Uint8Array,
  needle: Uint8Array,
  start = 0,
): number {
  outer: for (let i = start; i <= haystack.length - needle.length; i++) {
    for (let j = 0; j < needle.length; j++) {
      if (haystack[i + j] !== needle[j]) continue outer;
    }
    return i;
  }
  return -1;
}

/**
 * Get all encrypted thumbnails by attachment UUID.
 * Returns thumbnails at all available sizes.
 * @deprecated Use getAttachmentThumbnail for single-size fetches
 */
export async function getAttachmentThumbnails(
  uuid: string,
): Promise<ThumbnailsResponse> {
  const response = await fetch(`${API_PATH}/attachments/${uuid}/thumbnails`, {
    credentials: "include",
  });

  if (!response.ok) {
    const errorMsg = await getErrorMessage(
      response,
      "Failed to get thumbnails",
    );
    throw new Error(errorMsg);
  }

  // Parse multipart response
  const contentType = response.headers.get("Content-Type") || "";
  const boundaryMatch = contentType.match(/boundary=(.+)/);
  if (!boundaryMatch) {
    throw new Error("Invalid multipart response: missing boundary");
  }
  const boundary = boundaryMatch[1];

  const arrayBuffer = await response.arrayBuffer();
  const bytes = new Uint8Array(arrayBuffer);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  const boundaryBytes = encoder.encode(`--${boundary}`);
  const headerSeparator = encoder.encode("\r\n\r\n");

  const result: ThumbnailsResponse = {
    sm: { data: new ArrayBuffer(0), iv: "" },
  };

  // Find all boundary positions
  let pos = 0;
  while (pos < bytes.length) {
    const boundaryStart = findBytes(bytes, boundaryBytes, pos);
    if (boundaryStart === -1) break;

    // Move past the boundary
    let partStart = boundaryStart + boundaryBytes.length;

    // Check for closing boundary (--)
    if (bytes[partStart] === 0x2d && bytes[partStart + 1] === 0x2d) {
      break;
    }

    // Skip CRLF after boundary
    if (bytes[partStart] === 0x0d && bytes[partStart + 1] === 0x0a) {
      partStart += 2;
    }

    // Find header/body separator
    const headerEnd = findBytes(bytes, headerSeparator, partStart);
    if (headerEnd === -1) {
      pos = partStart;
      continue;
    }

    // Parse headers (ASCII safe to decode as text)
    const headerBytes = bytes.slice(partStart, headerEnd);
    const headerText = decoder.decode(headerBytes);

    const sizeMatch = headerText.match(/X-Thumbnail-Size:\s*(\w+)/i);
    const ivMatch = headerText.match(/X-Encryption-IV:\s*([^\r\n]+)/i);

    if (!sizeMatch || !ivMatch) {
      pos = headerEnd + 4;
      continue;
    }

    const size = sizeMatch[1] as "sm" | "md" | "lg";
    const iv = ivMatch[1].trim();

    // Body starts after header separator
    const bodyStart = headerEnd + 4;

    // Find the next boundary to determine body end
    const nextBoundary = findBytes(bytes, boundaryBytes, bodyStart);
    let bodyEnd: number;
    if (nextBoundary === -1) {
      bodyEnd = bytes.length;
    } else {
      // Body ends before CRLF preceding next boundary
      bodyEnd = nextBoundary;
      if (
        bodyEnd >= 2 &&
        bytes[bodyEnd - 2] === 0x0d &&
        bytes[bodyEnd - 1] === 0x0a
      ) {
        bodyEnd -= 2;
      }
    }

    // Extract binary body data
    const bodyData = arrayBuffer.slice(bodyStart, bodyEnd);

    const thumbnailData: ThumbnailData = { data: bodyData, iv };

    if (size === "sm") {
      result.sm = thumbnailData;
    } else if (size === "md") {
      result.md = thumbnailData;
    } else if (size === "lg") {
      result.lg = thumbnailData;
    }

    pos = nextBoundary === -1 ? bytes.length : nextBoundary;
  }
  console.log(result);

  return result;
}
