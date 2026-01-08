/**
 * Gallery widget decorations for CodeMirror.
 *
 * Handles image galleries with the format:
 * ::gallery{}![alt](attachment:uuid1)![alt](attachment:uuid2)::
 *
 * - Multiple images displayed in a row with individual delete buttons
 * - Cursor can be positioned between images
 * - Backspace deletes the image before cursor
 * - When last image is deleted, the whole gallery is removed
 * - Empty JSON {} reserved for future styling options
 */

import { StateField } from "@codemirror/state";
import {
	Decoration,
	type DecorationSet,
	EditorView,
	keymap,
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

// Cache for decrypted full images (uuid -> blob URL)
const fullImageCache = new Map<string, string>();

/**
 * Determine optimal thumbnail size based on window width and device pixel ratio.
 */
function getOptimalThumbnailSize(): ThumbnailSize {
	const width = window.innerWidth;
	const height = window.innerHeight;

	if (width <= 600) return "sm";
	if (width > 1600 && height > 1600) return "lg";
	return "md";
}

// ============================================================================
// Gallery Widgets
// ============================================================================

/** Hidden widget for ::gallery{} prefix */
class GalleryStartWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const span = document.createElement("span");
		span.className = "cm-gallery-start";
		return span;
	}

	ignoreEvent(): boolean {
		return false;
	}
}

/** Hidden widget for :: suffix */
class GalleryEndWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const span = document.createElement("span");
		span.className = "cm-gallery-end";
		return span;
	}

	ignoreEvent(): boolean {
		return false;
	}
}

/** Widget for an image inside a gallery (with delete button) */
class GalleryImageWidget extends WidgetType {
	constructor(
		private uuid: string,
		private alt: string,
		private from: number,
		private to: number,
		private galleryStart: number,
		private galleryEnd: number,
		private imageCount: number,
	) {
		super();
	}

	eq(other: GalleryImageWidget): boolean {
		return this.uuid === other.uuid && this.alt === other.alt;
	}

	toDOM(view: EditorView): HTMLElement {
		const container = document.createElement("span");
		container.className = "cm-gallery-image";
		this.renderThumbnail(container, view);
		return container;
	}

	private async renderThumbnail(container: HTMLElement, view: EditorView): Promise<void> {
		const cached = thumbnailCache.get(this.uuid);
		if (cached) {
			this.displayThumbnail(container, cached, view);
			return;
		}

		const loading = document.createElement("span");
		loading.className = "cm-attachment-loading";
		loading.textContent = "Loading...";
		container.appendChild(loading);

		try {
			const mek = getSessionEncryptionKey();
			if (!mek) {
				loading.textContent = "Unlock required";
				loading.className = "cm-attachment-error";
				return;
			}

			const response = await getAttachmentThumbnail(this.uuid, getOptimalThumbnailSize());
			const decrypted = await decryptBinary(response.data, response.iv, mek);
			const blob = new Blob([decrypted], { type: "image/jpeg" });
			const blobUrl = URL.createObjectURL(blob);

			thumbnailCache.set(this.uuid, blobUrl);
			container.removeChild(loading);
			this.displayThumbnail(container, blobUrl, view);
		} catch (err) {
			console.error("Failed to load thumbnail:", err);
			loading.textContent = "Failed to load";
			loading.className = "cm-attachment-error";
		}
	}

	private displayThumbnail(container: HTMLElement, src: string, view: EditorView): void {
		const wrapper = document.createElement("span");
		wrapper.className = "cm-attachment-thumbnail-wrapper";

		const img = document.createElement("img");
		img.src = src;
		img.alt = this.alt || "Attached image (click to enlarge)";
		img.className = "cm-attachment-thumbnail";
		img.title = "Click to view full size";

		img.addEventListener("click", () => this.showFullImage());

		wrapper.appendChild(img);
		container.appendChild(wrapper);

		this.addDeleteButton(container, view);
	}

	private addDeleteButton(container: HTMLElement, view: EditorView): void {
		const deleteBtn = document.createElement("button");
		deleteBtn.type = "button";
		deleteBtn.className = "cm-gallery-delete-btn";
		deleteBtn.setAttribute("aria-label", "Delete image");
		deleteBtn.setAttribute("tabindex", "0");
		deleteBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>`;

		deleteBtn.addEventListener("click", (e) => {
			e.stopPropagation();
			this.deleteImage(view);
		});

		deleteBtn.addEventListener("keydown", (e) => {
			if (e.key === "Enter" || e.key === " ") {
				e.preventDefault();
				e.stopPropagation();
				this.deleteImage(view);
			}
		});

		container.appendChild(deleteBtn);
	}

	private deleteImage(view: EditorView): void {
		if (this.imageCount === 1) {
			// Delete the entire gallery
			view.dispatch({
				changes: { from: this.galleryStart, to: this.galleryEnd, insert: "" },
			});
		} else {
			// Delete just this image
			view.dispatch({
				changes: { from: this.from, to: this.to, insert: "" },
			});
		}
	}

	private async showFullImage(): Promise<void> {
		const overlay = document.createElement("div");
		overlay.className = "cm-attachment-overlay";

		const closeHandler = (e: MouseEvent | KeyboardEvent) => {
			if (e instanceof KeyboardEvent && e.key !== "Escape") return;
			if (e instanceof MouseEvent && e.target !== overlay) return;
			overlay.remove();
			document.removeEventListener("keydown", closeHandler);
		};
		overlay.addEventListener("click", closeHandler);
		document.addEventListener("keydown", closeHandler);

		const cachedFull = fullImageCache.get(this.uuid);
		if (cachedFull) {
			this.displayFullImage(overlay, cachedFull);
			document.body.appendChild(overlay);
			return;
		}

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

			const response = await getAttachment(this.uuid);
			const decrypted = await decryptBinary(response.data, response.iv, mek);
			const blob = new Blob([decrypted], { type: "image/jpeg" });
			const blobUrl = URL.createObjectURL(blob);

			fullImageCache.set(this.uuid, blobUrl);
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
		img.addEventListener("click", (e) => e.stopPropagation());
		overlay.appendChild(img);
	}

	ignoreEvent(): boolean {
		return false;
	}
}

// ============================================================================
// Shared upload function
// ============================================================================

async function uploadImageFile(file: File): Promise<string> {
	const mek = getSessionEncryptionKey();
	if (!mek) {
		throw new Error("Encryption key not available. Please unlock first.");
	}

	const imageData = await file.arrayBuffer();
	const thumbnails = await generateThumbnails(file);

	const [encryptedImage, encThumbSm, encThumbMd, encThumbLg] = await Promise.all([
		encryptBinary(imageData, mek),
		encryptBinary(thumbnails.sm, mek),
		encryptBinary(thumbnails.md, mek),
		encryptBinary(thumbnails.lg, mek),
	]);

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

/**
 * Trigger an image upload via file picker.
 * Opens a file dialog, uploads the selected image, and inserts a gallery at cursor.
 */
export function triggerImageUpload(view: EditorView): void {
	const input = document.createElement("input");
	input.type = "file";
	input.accept = "image/*";

	input.addEventListener("change", async () => {
		const file = input.files?.[0];
		if (!file) return;

		const pos = view.state.selection.main.head;

		try {
			const uuid = await uploadImageFile(file);
			const gallery = `::gallery{}![image](attachment:${uuid})::`;
			view.dispatch({
				changes: { from: pos, to: pos, insert: gallery },
			});
		} catch (err) {
			console.error("Failed to upload image:", err);
		}
	});

	input.click();
}

// ============================================================================
// Patterns
// ============================================================================

// Pattern matches ::gallery{}...:: with images inside
const GALLERY_PATTERN = /::gallery\{([^}]*)\}((?:!\[[^\]]*\]\(attachment:[a-zA-Z0-9-]+\))+)::/g;

// Pattern for extracting individual images from gallery content
const GALLERY_IMAGE_PATTERN = /!\[([^\]]*)\]\(attachment:([a-zA-Z0-9-]+)\)/g;

// ============================================================================
// Find galleries in document (for backspace handling)
// ============================================================================

function findGalleryLines(doc: { lines: number; line: (n: number) => { text: string } }): Set<number> {
	const galleryLines = new Set<number>();

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		GALLERY_PATTERN.lastIndex = 0;
		if (GALLERY_PATTERN.test(line.text)) {
			galleryLines.add(i);
		}
	}

	return galleryLines;
}

/**
 * State field that caches gallery line numbers.
 * Galleries occupy entire lines, so we just track which lines have them.
 * Rescans on any document change (cheap since we're just checking line patterns).
 */
const galleryLines = StateField.define<Set<number>>({
	create(state) {
		return findGalleryLines(state.doc);
	},
	update(lines, tr) {
		if (tr.docChanged) {
			return findGalleryLines(tr.newDoc);
		}
		return lines;
	},
});

// ============================================================================
// Build decorations
// ============================================================================

function buildDecorations(view: EditorView): DecorationSet {
	const decorations: Array<{ from: number; to: number; deco: Decoration }> = [];
	const doc = view.state.doc;

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		let match: RegExpExecArray | null;
		GALLERY_PATTERN.lastIndex = 0;

		while ((match = GALLERY_PATTERN.exec(line.text)) !== null) {
			const galleryStart = line.from + match.index;
			const galleryEnd = galleryStart + match[0].length;
			const configPart = match[1];
			const imagesContent = match[2];

			// Calculate positions
			const prefixEnd = galleryStart + "::gallery{".length + configPart.length + "}".length;
			const suffixStart = galleryEnd - "::".length;

			// Add decoration for ::gallery{} prefix (hidden)
			decorations.push({
				from: galleryStart,
				to: prefixEnd,
				deco: Decoration.replace({
					widget: new GalleryStartWidget(),
				}),
			});

			// Find and add decorations for each image
			const imagesStart = prefixEnd;
			let imgMatch: RegExpExecArray | null;
			GALLERY_IMAGE_PATTERN.lastIndex = 0;

			const imagePositions: Array<{ from: number; to: number; alt: string; uuid: string }> = [];
			while ((imgMatch = GALLERY_IMAGE_PATTERN.exec(imagesContent)) !== null) {
				imagePositions.push({
					from: imagesStart + imgMatch.index,
					to: imagesStart + imgMatch.index + imgMatch[0].length,
					alt: imgMatch[1],
					uuid: imgMatch[2],
				});
			}

			const imageCount = imagePositions.length;
			for (const img of imagePositions) {
				decorations.push({
					from: img.from,
					to: img.to,
					deco: Decoration.replace({
						widget: new GalleryImageWidget(
							img.uuid,
							img.alt,
							img.from,
							img.to,
							galleryStart,
							galleryEnd,
							imageCount,
						),
					}),
				});
			}

			// Add decoration for :: suffix (hidden)
			decorations.push({
				from: suffixStart,
				to: galleryEnd,
				deco: Decoration.replace({
					widget: new GalleryEndWidget(),
				}),
			});
		}
	}

	// Sort by position for CodeMirror
	decorations.sort((a, b) => a.from - b.from);

	return Decoration.set(
		decorations.map((d) => d.deco.range(d.from, d.to)),
	);
}

const galleryViewPlugin = ViewPlugin.fromClass(
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

// ============================================================================
// Atomic ranges
// ============================================================================

function buildAtomicRanges(view: EditorView): DecorationSet {
	const ranges: Array<{ from: number; to: number }> = [];
	const doc = view.state.doc;

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		let match: RegExpExecArray | null;
		GALLERY_PATTERN.lastIndex = 0;

		while ((match = GALLERY_PATTERN.exec(line.text)) !== null) {
			const galleryStart = line.from + match.index;
			const galleryEnd = galleryStart + match[0].length;

			// Make the entire gallery atomic as one unit
			ranges.push({ from: galleryStart, to: galleryEnd });
		}
	}

	// Sort by position
	ranges.sort((a, b) => a.from - b.from);

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

// ============================================================================
// Keyboard handler for galleries
// ============================================================================

/** Context for gallery keyboard handlers */
interface GalleryContext {
	view: EditorView;
	doc: EditorView["state"]["doc"];
	cursorLine: { number: number; from: number; to: number; text: string };
	galleries: Set<number>;
	from: number;
}

/** Get context for gallery handlers, or null if selection is not collapsed */
function getGalleryContext(view: EditorView): GalleryContext | null {
	const { from, to } = view.state.selection.main;
	if (from !== to) return null;

	return {
		view,
		doc: view.state.doc,
		cursorLine: view.state.doc.lineAt(from),
		galleries: view.state.field(galleryLines),
		from,
	};
}

/** Swap current line with gallery line above it */
function swapWithGalleryAbove(ctx: GalleryContext): boolean {
	const { view, doc, cursorLine } = ctx;
	if (cursorLine.number <= 1) return false;

	const prevLineNum = cursorLine.number - 1;
	if (!ctx.galleries.has(prevLineNum)) return false;

	const prevLine = doc.line(prevLineNum);
	view.dispatch({
		changes: {
			from: prevLine.from,
			to: cursorLine.to,
			insert: cursorLine.text + "\n" + prevLine.text,
		},
		selection: { anchor: prevLine.from },
	});
	return true;
}

/** Swap current line with gallery line below it */
function swapWithGalleryBelow(ctx: GalleryContext): boolean {
	const { view, doc, cursorLine } = ctx;
	if (cursorLine.number >= doc.lines) return false;

	const nextLineNum = cursorLine.number + 1;
	if (!ctx.galleries.has(nextLineNum)) return false;

	const nextLine = doc.line(nextLineNum);
	view.dispatch({
		changes: {
			from: cursorLine.from,
			to: nextLine.to,
			insert: nextLine.text + "\n" + cursorLine.text,
		},
		selection: { anchor: cursorLine.from + nextLine.text.length + 1 + cursorLine.text.length },
	});
	return true;
}

/** Skip over gallery above (for up/left navigation) */
function skipGalleryAbove(ctx: GalleryContext): boolean {
	const { view, doc, cursorLine } = ctx;
	if (cursorLine.number <= 1) return false;

	const prevLineNum = cursorLine.number - 1;
	if (!ctx.galleries.has(prevLineNum)) return false;

	if (prevLineNum > 1) {
		view.dispatch({ selection: { anchor: doc.line(prevLineNum - 1).to } });
	} else {
		view.dispatch({ selection: { anchor: 0 } });
	}
	return true;
}

/** Skip over gallery below (for down/right navigation) */
function skipGalleryBelow(ctx: GalleryContext): boolean {
	const { view, doc, cursorLine } = ctx;
	if (cursorLine.number >= doc.lines) return false;

	const nextLineNum = cursorLine.number + 1;
	if (!ctx.galleries.has(nextLineNum)) return false;

	if (nextLineNum < doc.lines) {
		view.dispatch({ selection: { anchor: doc.line(nextLineNum + 1).from } });
	} else {
		view.dispatch({ selection: { anchor: doc.length } });
	}
	return true;
}

/**
 * Custom keyboard handler for gallery images.
 * Prevents editing gallery lines and handles navigation around them.
 */
const galleryKeyHandler = keymap.of([
	{
		key: "Backspace",
		run(view) {
			const ctx = getGalleryContext(view);
			if (!ctx) return false;

			// On gallery line at end: move to start
			if (ctx.galleries.has(ctx.cursorLine.number) && ctx.from === ctx.cursorLine.to) {
				view.dispatch({ selection: { anchor: ctx.cursorLine.from } });
				return true;
			}

			// At start of line after gallery: swap lines
			if (ctx.from === ctx.cursorLine.from) {
				return swapWithGalleryAbove(ctx);
			}

			return false;
		},
	},
	{
		key: "Delete",
		run(view) {
			const ctx = getGalleryContext(view);
			if (!ctx) return false;

			// On gallery line at start: move to end
			if (ctx.galleries.has(ctx.cursorLine.number) && ctx.from === ctx.cursorLine.from) {
				view.dispatch({ selection: { anchor: ctx.cursorLine.to } });
				return true;
			}

			// At end of line before gallery: swap lines
			if (ctx.from === ctx.cursorLine.to) {
				return swapWithGalleryBelow(ctx);
			}

			return false;
		},
	},
	{
		key: "Mod-Backspace",
		run(view) {
			const ctx = getGalleryContext(view);
			if (!ctx) return false;

			// On gallery line: move to start
			if (ctx.galleries.has(ctx.cursorLine.number)) {
				view.dispatch({ selection: { anchor: ctx.cursorLine.from } });
				return true;
			}

			// At start of line after gallery: swap lines
			if (ctx.from === ctx.cursorLine.from) {
				return swapWithGalleryAbove(ctx);
			}

			return false;
		},
	},
	{
		key: "ArrowUp",
		run(view) {
			const ctx = getGalleryContext(view);
			return ctx ? skipGalleryAbove(ctx) : false;
		},
	},
	{
		key: "ArrowDown",
		run(view) {
			const ctx = getGalleryContext(view);
			return ctx ? skipGalleryBelow(ctx) : false;
		},
	},
	{
		key: "ArrowLeft",
		run(view) {
			const ctx = getGalleryContext(view);
			if (!ctx) return false;
			// Only skip when at start of line
			return ctx.from === ctx.cursorLine.from ? skipGalleryAbove(ctx) : false;
		},
	},
	{
		key: "ArrowRight",
		run(view) {
			const ctx = getGalleryContext(view);
			if (!ctx) return false;
			// Only skip when at end of line
			return ctx.from === ctx.cursorLine.to ? skipGalleryBelow(ctx) : false;
		},
	},
]);

/**
 * Update listener that ensures cursor doesn't end up inside a gallery.
 * If cursor is on a gallery line, move it to the end (before newline).
 * Only runs when selection changes WITHOUT a doc change (click/arrow navigation).
 */
const galleryCursorGuard = EditorView.updateListener.of((update) => {
	// Only guard on pure selection changes (clicks, arrow keys)
	// Not when typing (docChanged) - the atomic ranges handle that
	if (!update.selectionSet || update.docChanged) return;

	const { from, to } = update.state.selection.main;

	// Only handle collapsed selections (cursor, not range)
	if (from !== to) return;

	const doc = update.state.doc;
	const cursorLine = doc.lineAt(from);
	const galleries = update.state.field(galleryLines);

	// If cursor is on a gallery line, always move to end of line
	if (galleries.has(cursorLine.number) && from !== cursorLine.to) {
		update.view.dispatch({
			selection: { anchor: cursorLine.to },
		});
	}
});

/**
 * Input handler that redirects typing on gallery lines to the next line.
 * When user types on a gallery line, insert a newline first, then the text.
 */
const galleryInputHandler = EditorView.inputHandler.of((view, from, to, text) => {
	// Only handle text input (not deletions)
	if (!text) return false;

	const doc = view.state.doc;
	const cursorLine = doc.lineAt(from);
	const galleries = view.state.field(galleryLines);

	// If typing on a gallery line, redirect to next line
	if (galleries.has(cursorLine.number)) {
		// Insert at end of gallery line with a newline prefix
		view.dispatch({
			changes: { from: cursorLine.to, to: cursorLine.to, insert: "\n" + text },
			selection: { anchor: cursorLine.to + 1 + text.length },
		});
		return true;
	}

	return false;
});

// ============================================================================
// Export
// ============================================================================

/**
 * Gallery plugin that provides:
 * - Widget decorations for gallery images
 * - Atomic range behavior
 * - Custom backspace handling
 * - Input redirection (typing on gallery lines goes to next line)
 * - Cached gallery line numbers (via state field)
 */
export const attachmentPlugin = [
	galleryLines,
	galleryViewPlugin,
	atomicRangesPlugin,
	galleryKeyHandler,
	galleryCursorGuard,
	galleryInputHandler,
];

/**
 * Parse attachment UUIDs from content.
 * Used when saving posts to update reference counts.
 */
export function parseAttachmentUuids(content: string): string[] {
	const uuids: string[] = [];
	let match: RegExpExecArray | null;
	const pattern = /!\[[^\]]*\]\(attachment:([a-f0-9-]+)\)/g;

	while ((match = pattern.exec(content)) !== null) {
		if (match[1] !== "pending") {
			uuids.push(match[1]);
		}
	}

	return [...new Set(uuids)];
}

/**
 * Clear all image caches.
 */
export function clearImageCache(): void {
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
