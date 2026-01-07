/**
 * Attachment widget decorations for CodeMirror.
 *
 * Handles inline image attachments with the format ![alt](attachment:uuid).
 * - "attachment:pending" shows a file picker button
 * - "attachment:<uuid>" fetches and displays the decrypted image
 */

import {
	Decoration,
	type DecorationSet,
	EditorView,
	ViewPlugin,
	type ViewUpdate,
	WidgetType,
} from "@codemirror/view";

import { getAttachment, uploadAttachment } from "../api/attachments.ts";
import {
	ENCRYPTED_FORMAT_VERSION,
	decryptBinary,
	encryptBinary,
	fromBase64Url,
	toBase64Url,
} from "../crypto/operations.ts";
import { getSessionEncryptionKey } from "../crypto/keystore.ts";
import { generateThumbnail } from "./thumbnail.ts";

// Cache for decrypted images to avoid repeated fetching/decryption
const imageCache = new Map<string, string>();

class AttachmentWidget extends WidgetType {
	private container: HTMLElement | null = null;

	constructor(
		private uuid: string,
		private alt: string,
		private from: number,
		private to: number,
	) {
		super();
	}

	eq(other: AttachmentWidget): boolean {
		return this.uuid === other.uuid && this.alt === other.alt;
	}

	toDOM(view: EditorView): HTMLElement {
		const container = document.createElement("div");
		container.className = "cm-attachment-widget";
		this.container = container;

		if (this.uuid === "pending") {
			this.renderFilePicker(container, view);
		} else {
			this.renderImage(container);
		}

		return container;
	}

	private renderFilePicker(container: HTMLElement, view: EditorView): void {
		const wrapper = document.createElement("div");
		wrapper.className = "cm-attachment-picker";

		const btn = document.createElement("button");
		btn.type = "button";
		btn.textContent = "Select Image";
		btn.className = "cm-attachment-picker-btn";

		const input = document.createElement("input");
		input.type = "file";
		input.accept = "image/*";
		input.style.display = "none";

		input.addEventListener("change", async () => {
			const file = input.files?.[0];
			if (!file) return;

			btn.textContent = "Uploading...";
			btn.disabled = true;

			try {
				const uuid = await this.uploadImage(file);
				// Replace the markdown with the new UUID
				const newText = `![${this.alt || "image"}](attachment:${uuid})`;
				view.dispatch({
					changes: { from: this.from, to: this.to, insert: newText },
				});
			} catch (err) {
				console.error("Failed to upload image:", err);
				btn.textContent = "Upload Failed - Retry";
				btn.disabled = false;
			}
		});

		btn.addEventListener("click", () => {
			input.click();
		});

		wrapper.appendChild(btn);
		wrapper.appendChild(input);
		container.appendChild(wrapper);
	}

	private async uploadImage(file: File): Promise<string> {
		const mek = getSessionEncryptionKey();
		if (!mek) {
			throw new Error("Encryption key not available. Please unlock first.");
		}

		// Read file as ArrayBuffer
		const imageData = await file.arrayBuffer();

		// Generate thumbnail
		const thumbnailData = await generateThumbnail(file);

		// Encrypt both image and thumbnail
		const [encryptedImage, encryptedThumbnail] = await Promise.all([
			encryptBinary(imageData, mek),
			encryptBinary(thumbnailData, mek),
		]);

		// Upload to server
		const response = await uploadAttachment({
			encrypted_image: toBase64Url(encryptedImage.ciphertext),
			encrypted_image_iv: encryptedImage.iv,
			encrypted_thumbnail: toBase64Url(encryptedThumbnail.ciphertext),
			encrypted_thumbnail_iv: encryptedThumbnail.iv,
			encryption_version: ENCRYPTED_FORMAT_VERSION,
		});

		return response.uuid;
	}

	private async renderImage(container: HTMLElement): Promise<void> {
		// Check cache first
		const cached = imageCache.get(this.uuid);
		if (cached) {
			this.displayImage(container, cached);
			return;
		}

		// Show loading state
		const loading = document.createElement("div");
		loading.className = "cm-attachment-loading";
		loading.textContent = "Loading image...";
		container.appendChild(loading);

		try {
			const mek = getSessionEncryptionKey();
			if (!mek) {
				loading.textContent = "Unlock required to view image";
				loading.className = "cm-attachment-error";
				return;
			}

			// Fetch encrypted image
			const response = await getAttachment(this.uuid);

			// Decrypt
			const ciphertext = fromBase64Url(response.encrypted_image);
			const decrypted = await decryptBinary(ciphertext, response.iv, mek);

			// Create blob URL
			const blob = new Blob([decrypted], { type: "image/jpeg" });
			const blobUrl = URL.createObjectURL(blob);

			// Cache the blob URL
			imageCache.set(this.uuid, blobUrl);

			// Display image
			container.removeChild(loading);
			this.displayImage(container, blobUrl);
		} catch (err) {
			console.error("Failed to load image:", err);
			loading.textContent = "Failed to load image";
			loading.className = "cm-attachment-error";
		}
	}

	private displayImage(container: HTMLElement, src: string): void {
		const img = document.createElement("img");
		img.src = src;
		img.alt = this.alt || "Attached image";
		img.className = "cm-attachment-image";
		container.appendChild(img);
	}

	destroy(): void {
		this.container = null;
	}

	ignoreEvent(): boolean {
		return false;
	}
}

// Pattern matches ![alt text](attachment:uuid)
const ATTACHMENT_PATTERN = /!\[([^\]]*)\]\(attachment:([a-zA-Z0-9-]+)\)/g;

function buildDecorations(view: EditorView): DecorationSet {
	const decorations: Array<{ from: number; to: number; deco: Decoration }> = [];
	const doc = view.state.doc;

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		let match: RegExpExecArray | null;
		ATTACHMENT_PATTERN.lastIndex = 0;

		while ((match = ATTACHMENT_PATTERN.exec(line.text)) !== null) {
			const alt = match[1];
			const uuid = match[2];
			const from = line.from + match.index;
			const to = from + match[0].length;

			decorations.push({
				from,
				to,
				deco: Decoration.replace({
					widget: new AttachmentWidget(uuid, alt, from, to),
				}),
			});
		}
	}

	return Decoration.set(
		decorations.map((d) => d.deco.range(d.from, d.to)),
	);
}

export const attachmentPlugin = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = buildDecorations(view);
		}

		update(update: ViewUpdate) {
			if (update.docChanged || update.viewportChanged) {
				this.decorations = buildDecorations(update.view);
			}
		}
	},
	{
		decorations: (v) => v.decorations,
	},
);

/**
 * Parse attachment UUIDs from content.
 * Used when saving posts to update reference counts.
 */
export function parseAttachmentUuids(content: string): string[] {
	const uuids: string[] = [];
	let match: RegExpExecArray | null;
	const pattern = /!\[[^\]]*\]\(attachment:([a-f0-9-]+)\)/g;

	while ((match = pattern.exec(content)) !== null) {
		// Skip "pending" entries
		if (match[1] !== "pending") {
			uuids.push(match[1]);
		}
	}

	// Return unique UUIDs
	return [...new Set(uuids)];
}

/**
 * Clear the image cache.
 * Call when logging out or when encryption key changes.
 */
export function clearImageCache(): void {
	// Revoke all blob URLs to free memory
	for (const url of imageCache.values()) {
		URL.revokeObjectURL(url);
	}
	imageCache.clear();
}

/**
 * Clear cached images except for the specified UUIDs.
 * Call when navigating away from a post to clean up deleted images.
 */
export function clearImageCacheExcept(keepUuids: string[]): void {
	const keepSet = new Set(keepUuids);
	for (const [uuid, url] of imageCache.entries()) {
		if (!keepSet.has(uuid)) {
			URL.revokeObjectURL(url);
			imageCache.delete(uuid);
		}
	}
}
