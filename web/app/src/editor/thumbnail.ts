/**
 * Thumbnail generation using canvas.
 * Creates JPEG thumbnails with a maximum dimension of 200px.
 */

const MAX_THUMBNAIL_SIZE = 200;
const THUMBNAIL_QUALITY = 0.8;

/**
 * Generate a thumbnail from an image file.
 * Returns the thumbnail as a JPEG ArrayBuffer.
 */
export async function generateThumbnail(file: File): Promise<ArrayBuffer> {
	// Create image element from file
	const img = await loadImage(file);

	// Calculate scaled dimensions
	const { width, height } = calculateThumbnailSize(
		img.naturalWidth,
		img.naturalHeight,
	);

	// Create canvas and draw scaled image
	const canvas = document.createElement("canvas");
	canvas.width = width;
	canvas.height = height;

	const ctx = canvas.getContext("2d");
	if (!ctx) {
		throw new Error("Failed to get canvas 2D context");
	}

	ctx.drawImage(img, 0, 0, width, height);

	// Export as JPEG
	const blob = await new Promise<Blob>((resolve, reject) => {
		canvas.toBlob(
			(b) => {
				if (b) {
					resolve(b);
				} else {
					reject(new Error("Failed to create thumbnail blob"));
				}
			},
			"image/jpeg",
			THUMBNAIL_QUALITY,
		);
	});

	return blob.arrayBuffer();
}

/**
 * Load an image from a File object.
 */
function loadImage(file: File): Promise<HTMLImageElement> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		const url = URL.createObjectURL(file);

		img.onload = () => {
			URL.revokeObjectURL(url);
			resolve(img);
		};

		img.onerror = () => {
			URL.revokeObjectURL(url);
			reject(new Error("Failed to load image"));
		};

		img.src = url;
	});
}

/**
 * Calculate thumbnail dimensions while maintaining aspect ratio.
 * Neither dimension will exceed MAX_THUMBNAIL_SIZE.
 */
function calculateThumbnailSize(
	originalWidth: number,
	originalHeight: number,
): { width: number; height: number } {
	// If image is already small enough, keep original size
	if (
		originalWidth <= MAX_THUMBNAIL_SIZE &&
		originalHeight <= MAX_THUMBNAIL_SIZE
	) {
		return { width: originalWidth, height: originalHeight };
	}

	// Scale down to fit within MAX_THUMBNAIL_SIZE
	const aspectRatio = originalWidth / originalHeight;

	if (originalWidth > originalHeight) {
		// Landscape
		return {
			width: MAX_THUMBNAIL_SIZE,
			height: Math.round(MAX_THUMBNAIL_SIZE / aspectRatio),
		};
	} else {
		// Portrait or square
		return {
			width: Math.round(MAX_THUMBNAIL_SIZE * aspectRatio),
			height: MAX_THUMBNAIL_SIZE,
		};
	}
}
