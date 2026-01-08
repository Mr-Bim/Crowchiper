/**
 * Attachment widget decorations for CodeMirror.
 *
 * Handles inline image attachments with the format ![alt](attachment:uuid).
 * - "attachment:pending" shows a file picker button
 * - "attachment:<uuid>" fetches and displays the decrypted thumbnail with srcset
 * - Clicking the thumbnail loads and displays the full image
 */

import {
	Decoration,
	type DecorationSet,
	EditorView,
	ViewPlugin,
	type ViewUpdate,
	WidgetType,
} from "@codemirror/view";

import { getAttachment, getAttachmentThumbnail, uploadAttachment, type ThumbnailSize } from "../api/attachments.ts";
import {
	ENCRYPTED_FORMAT_VERSION,
	decryptBinary,
	encryptBinary,
} from "../crypto/operations.ts";
import { getSessionEncryptionKey } from "../crypto/keystore.ts";
import { generateThumbnails } from "./thumbnail.ts";

// Cache for decrypted thumbnails (uuid -> blob URL)
const thumbnailCache = new Map<string, string>();

/**
 * Determine optimal thumbnail size based on window width and device pixel ratio.
 * - sm (200px): mobile devices or small windows
 * - md (400px): regular laptops and tablets
 * - lg (800px): large screens / high DPI displays
 */
function getOptimalThumbnailSize(): ThumbnailSize {
	const width = window.innerWidth;
  const height = window.innerHeight;

	// Effective pixel width accounting for DPR

	// Mobile or small window: use small thumbnail
	if (width <= 600) return "sm";

	// Large screen or high DPI: use large thumbnail
	if (width > 1600 && height > 1600) return "lg";

	// Default: medium thumbnail for regular laptops
	return "md";
}

// Cache for decrypted full images (uuid -> blob URL)
const fullImageCache = new Map<string, string>();

class AttachmentWidget extends WidgetType {

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

		if (this.uuid === "pending") {
			this.renderFilePicker(container, view);
		} else {
			this.renderThumbnail(container);
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

		// Generate thumbnails at all sizes
		const thumbnails = await generateThumbnails(file);

		// Encrypt image and all thumbnails
		const [encryptedImage, encThumbSm, encThumbMd, encThumbLg] = await Promise.all([
			encryptBinary(imageData, mek),
			encryptBinary(thumbnails.sm, mek),
			encryptBinary(thumbnails.md, mek),
			encryptBinary(thumbnails.lg, mek),
		]);

		// Upload to server using binary streaming
		const response = await uploadAttachment({
			image: encryptedImage.ciphertext,
			image_iv: encryptedImage.iv,
			thumb_sm: encThumbSm.ciphertext,
			thumb_sm_iv: encThumbSm.iv,
			thumb_md: encThumbMd.ciphertext,
			thumb_md_iv: encThumbMd.iv,
			thumb_lg: encThumbLg.ciphertext,
			thumb_lg_iv: encThumbLg.iv,
			encryption_version: ENCRYPTED_FORMAT_VERSION,
		});

		return response.uuid;
	}

	private async renderThumbnail(container: HTMLElement): Promise<void> {
		// Check thumbnail cache first
		const cached = thumbnailCache.get(this.uuid);
		if (cached) {
			this.displayThumbnail(container, cached);
			return;
		}

		// Show loading state
		const loading = document.createElement("div");
		loading.className = "cm-attachment-loading";
		loading.textContent = "Loading...";
		container.appendChild(loading);

		try {
			const mek = getSessionEncryptionKey();
			if (!mek) {
				loading.textContent = "Unlock required to view image";
				loading.className = "cm-attachment-error";
				return;
			}

			// Fetch only the optimal size for editor display
			const response = await getAttachmentThumbnail(this.uuid, getOptimalThumbnailSize());

			// Decrypt the thumbnail
			const decrypted = await decryptBinary(response.data, response.iv, mek);
			const blob = new Blob([decrypted], { type: "image/jpeg" });
			const blobUrl = URL.createObjectURL(blob);

			// Cache the blob URL
			thumbnailCache.set(this.uuid, blobUrl);

			// Display thumbnail
			container.removeChild(loading);
			this.displayThumbnail(container, blobUrl);
		} catch (err) {
			console.error("Failed to load thumbnail:", err);
			loading.textContent = "Failed to load image";
			loading.className = "cm-attachment-error";
		}
	}

	private displayThumbnail(container: HTMLElement, src: string): void {
		const wrapper = document.createElement("div");
		wrapper.className = "cm-attachment-thumbnail-wrapper";

		const img = document.createElement("img");
		img.src = src;
		img.alt = this.alt || "Attached image (click to enlarge)";
		img.className = "cm-attachment-thumbnail";
		img.title = "Click to view full size";

		img.addEventListener("click", () => {
			this.showFullImage();
		});

		wrapper.appendChild(img);
		container.appendChild(wrapper);
	}

	private async showFullImage(): Promise<void> {
		// Create overlay
		const overlay = document.createElement("div");
		overlay.className = "cm-attachment-overlay";

		// Close on click outside or escape
		const closeHandler = (e: MouseEvent | KeyboardEvent) => {
			if (e instanceof KeyboardEvent && e.key !== "Escape") return;
			if (e instanceof MouseEvent && e.target !== overlay) return;
			overlay.remove();
			document.removeEventListener("keydown", closeHandler);
		};
		overlay.addEventListener("click", closeHandler);
		document.addEventListener("keydown", closeHandler);

		// Check full image cache first
		const cachedFull = fullImageCache.get(this.uuid);
		if (cachedFull) {
			this.displayFullImage(overlay, cachedFull);
			document.body.appendChild(overlay);
			return;
		}

		// Show loading state in overlay
		const loading = document.createElement("div");
		loading.className = "cm-attachment-overlay-loading";
		loading.textContent = "Loading full image...";
		overlay.appendChild(loading);
		document.body.appendChild(overlay);

		try {
			const mek = getSessionEncryptionKey();
			if (!mek) {
				loading.textContent = "Unlock required to view image";
				return;
			}

			// Fetch full encrypted image
			const response = await getAttachment(this.uuid);

			// Decrypt
			const decrypted = await decryptBinary(response.data, response.iv, mek);

			// Create blob URL
			const blob = new Blob([decrypted], { type: "image/jpeg" });
			const blobUrl = URL.createObjectURL(blob);

			// Cache the full image blob URL
			fullImageCache.set(this.uuid, blobUrl);

			// Display full image
			overlay.removeChild(loading);
			this.displayFullImage(overlay, blobUrl);
		} catch (err) {
			console.error("Failed to load full image:", err);
			loading.textContent = "Failed to load image";
		}
	}

	private displayFullImage(overlay: HTMLElement, src: string): void {
		const img = document.createElement("img");
		img.src = src;
		img.alt = this.alt || "Attached image";
		img.className = "cm-attachment-full-image";

		// Clicking the image itself shouldn't close the overlay
		img.addEventListener("click", (e) => {
			e.stopPropagation();
		});

		overlay.appendChild(img);
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

const attachmentViewPlugin = ViewPlugin.fromClass(
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
 * Build atomic ranges for attachments so they delete as a unit.
 * When the user deletes any character within the attachment markdown,
 * the entire syntax is deleted.
 */
function buildAtomicRanges(view: EditorView): DecorationSet {
	const ranges: Array<{ from: number; to: number }> = [];
	const doc = view.state.doc;

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		let match: RegExpExecArray | null;
		ATTACHMENT_PATTERN.lastIndex = 0;

		while ((match = ATTACHMENT_PATTERN.exec(line.text)) !== null) {
			const from = line.from + match.index;
			const to = from + match[0].length;
			ranges.push({ from, to });
		}
	}

	return Decoration.set(
		ranges.map((r) => Decoration.mark({ atomic: true }).range(r.from, r.to)),
	);
}

const atomicRangesPlugin = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = buildAtomicRanges(view);
		}

		update(update: ViewUpdate) {
			if (update.docChanged || update.viewportChanged) {
				this.decorations = buildAtomicRanges(update.view);
			}
		}
	},
	{
		decorations: (v) => v.decorations,
		provide: (plugin) =>
			EditorView.atomicRanges.of((view) => {
				const value = view.plugin(plugin);
				return value?.decorations ?? Decoration.none;
			}),
	},
);

/**
 * Combined attachment plugin that provides both the widget decorations
 * and atomic range behavior.
 */
export const attachmentPlugin = [attachmentViewPlugin, atomicRangesPlugin];

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
 * Clear all image caches.
 * Call when logging out or when encryption key changes.
 */
export function clearImageCache(): void {
	// Revoke all blob URLs to free memory
	for (const url of thumbnailCache.values()) {
		URL.revokeObjectURL(url);
	}
	thumbnailCache.clear();

	for (const url of fullImageCache.values()) {
		URL.revokeObjectURL(url);
	}
	fullImageCache.clear();
}

/**
 * Clear cached images except for the specified UUIDs.
 * Call when navigating away from a post to clean up deleted images.
 */
export function clearImageCacheExcept(keepUuids: string[]): void {
	const keepSet = new Set(keepUuids);

	for (const [uuid, url] of thumbnailCache.entries()) {
		if (!keepSet.has(uuid)) {
			URL.revokeObjectURL(url);
			thumbnailCache.delete(uuid);
		}
	}

	for (const [uuid, url] of fullImageCache.entries()) {
		if (!keepSet.has(uuid)) {
			URL.revokeObjectURL(url);
			fullImageCache.delete(uuid);
		}
	}
}
