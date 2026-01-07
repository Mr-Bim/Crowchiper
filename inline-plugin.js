import {
	existsSync,
	globSync,
	readFileSync,
	statSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { transform } from "lightningcss";

/**
 * Generate unique 2-letter class names (a-z only)
 * Returns a generator that yields unique names: aa, ab, ac, ..., zz
 */
function* generateClassNames() {
	const chars = "abcdefghijklmnopqrstuvwxyz";
	for (let i = 0; i < chars.length; i++) {
		for (let j = 0; j < chars.length; j++) {
			yield chars[i] + chars[j];
		}
	}
}

/**
 * Extract all class names from CSS content
 * Matches class selectors like .class-name
 */
function extractClassNames(css) {
	const classRegex = /\.([a-zA-Z_-][a-zA-Z0-9_-]*)/g;
	const classes = new Set();
	for (const match of css.matchAll(classRegex)) {
		classes.add(match[1]);
	}
	return classes;
}

/**
 * Split CSS into minifiable and non-minifiable sections based on markers.
 * .gl-minify-disable-NAME { --marker: 1 } turns OFF minification
 * .gl-minify-enable-NAME { --marker: 1 } turns it back ON
 * @param {string} css - CSS content
 * @returns {{minifiable: string, noMinify: string}}
 */
function splitCssByMarkers(css) {
	const markerStartRegex = /\.gl-minify-disable-[\w-]+\s*\{\s*--marker:\s*1\s*\}/g;
	const markerEndRegex = /\.gl-minify-enable-[\w-]+\s*\{\s*--marker:\s*1\s*\}/g;

	// Find all marker positions
	const markersStart = [...css.matchAll(markerStartRegex)].map((m) => ({
		type: "start",
		index: m.index,
		length: m[0].length,
	}));
	const markersEnd = [...css.matchAll(markerEndRegex)].map((m) => ({
		type: "end",
		index: m.index,
		length: m[0].length,
	}));

	// If no markers, everything is minifiable
	if (markersStart.length === 0 && markersEnd.length === 0) {
		return { minifiable: css, noMinify: "" };
	}

	// Combine and sort markers by position
	const allMarkers = [...markersStart, ...markersEnd].sort(
		(a, b) => a.index - b.index,
	);

	let minifiable = "";
	let noMinify = "";
	let lastIndex = 0;
	let inNoMinifySection = false;

	for (const marker of allMarkers) {
		// Content before this marker
		const content = css.slice(lastIndex, marker.index);

		if (inNoMinifySection) {
			noMinify += content;
		} else {
			minifiable += content;
		}

		// Toggle state based on marker type
		if (marker.type === "start") {
			inNoMinifySection = true;
		} else {
			inNoMinifySection = false;
		}

		// Skip past the marker itself
		lastIndex = marker.index + marker.length;
	}

	// Remaining content after last marker
	const remaining = css.slice(lastIndex);
	if (inNoMinifySection) {
		noMinify += remaining;
	} else {
		minifiable += remaining;
	}

	return { minifiable, noMinify };
}

/**
 * Minify class names in CSS and HTML
 * @param {string} css - CSS content
 * @param {string} html - HTML content
 * @param {Set<string>} excludeClasses - Class names to exclude from minification
 * @returns {{css: string, html: string, classMap: Map<string, string>}}
 */
function minifyClassNames(css, html, excludeClasses = new Set()) {
	const classNames = extractClassNames(css);
	const nameGen = generateClassNames();
	const classMap = new Map();

	// Build mapping of original -> minified names (excluding shared classes)
	for (const className of classNames) {
		// Skip classes that exist in styles.css (shared styles)
		if (excludeClasses.has(className)) {
			continue;
		}
		const minified = nameGen.next().value;
		if (!minified) {
			throw new Error("Ran out of class names (max 676)");
		}
		classMap.set(className, minified);
	}

	// Replace in CSS: .original-class -> .ab
	let minifiedCss = css;
	for (const [original, minified] of classMap) {
		// Match class selector, being careful about boundaries
		const cssRegex = new RegExp(
			`\\.${original.replace(/[-]/g, "\\-")}(?=[^a-zA-Z0-9_-]|$)`,
			"g",
		);
		minifiedCss = minifiedCss.replace(cssRegex, `.${minified}`);
	}

	// Replace in HTML: class="original-class" -> class="ab"
	let minifiedHtml = html;
	// Match class attributes and replace class names within them
	minifiedHtml = minifiedHtml.replace(
		/\bclass="([^"]*)"/g,
		(match, classValue) => {
			const classes = classValue.split(/\s+/);
			const minifiedClasses = classes.map((c) => classMap.get(c) || c);
			return `class="${minifiedClasses.join(" ")}"`;
		},
	);

	return { css: minifiedCss, html: minifiedHtml, classMap };
}

/**
 * Plugin to:
 * 1. Minify and copy styles.css to dist
 * 2. Inline small CSS files (<20KB) into HTML
 * 3. Inline IIFE script into HTML
 * 4. Replace asset path placeholders
 * 5. Minify CSS class names
 * 6. Minify HTML
 *
 * @param {Object} options
 * @param {string} options.assetsPath - The base path for assets (e.g., "/login" or "/fiery-sparrow")
 */
export function inlineIIFEPlugin(options = {}) {
	const { assetsPath } = options;

	return {
		name: "inline-iife",
		closeBundle() {
			const iifeFilePath = join(
				import.meta.dirname,
				"dist",
				"iife",
				"inline.js",
			);
			const buildType = process.env.BUILD || "login";
			const distDir = join(import.meta.dirname, "dist", buildType);

			try {
				const htmlFiles = globSync(`${distDir}/*.html`);
				const iifeContent = existsSync(iifeFilePath)
					? readFileSync(iifeFilePath, "utf-8")
					: "";
				const filesToDelete = new Set();

				// Build the pattern to match for CSS href replacement
				const assetsPattern = assetsPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

				// Minify and copy styles.css to dist
				const stylesPath = join(import.meta.dirname, "web", "styles.css");
				const stylesDistPath = join(distDir, "styles.css");
				let sharedClasses = new Set();
				if (existsSync(stylesPath)) {
					const stylesContent = readFileSync(stylesPath, "utf-8");
					// Extract class names from styles.css to exclude from minification
					sharedClasses = extractClassNames(stylesContent);
					const { code } = transform({
						filename: "styles.css",
						code: Buffer.from(stylesContent),
						minify: true,
					});
					writeFileSync(stylesDistPath, code);
					console.log(`✓ Minified styles.css`);
				}

				let totalClassesMinified = 0;

				for (const htmlFile of htmlFiles) {
					let html = readFileSync(htmlFile, "utf-8");
					let inlinedCss = "";
					let noMinifyCss = "";

					// Find and inline CSS files under 20KB
					const linkRegex =
						/<link[^>]*(?:rel="stylesheet"[^>]*href="([^"]+)"|href="([^"]+)"[^>]*rel="stylesheet")[^>]*>/g;

					let firstCssTag = null;
					for (const match of html.matchAll(linkRegex)) {
						const fullTag = match[0];
						const href = match[1] || match[2];

						// Build the full path to the CSS file
						const hrefPath = href.replace(new RegExp(`^${assetsPattern}/`), "");
						const cssFilePath = join(distDir, hrefPath);

						if (existsSync(cssFilePath)) {
							const cssContent = readFileSync(cssFilePath, "utf-8");
							const stats = statSync(cssFilePath);
							// Inline if under 20KB
							if (stats.size < 20 * 1024) {
								// Split CSS by markers - sections between marker:1 and marker:0 are not minified
								const { minifiable, noMinify } = splitCssByMarkers(cssContent);
								inlinedCss += minifiable;
								noMinifyCss += noMinify;

								// Track first CSS tag for replacement, remove others
								if (!firstCssTag) {
									firstCssTag = fullTag;
								} else {
									html = html.replace(fullTag, "");
								}
								filesToDelete.add(cssFilePath);
							}
						}
					}

					// Minify class names if we have inlined CSS (but not noMinifyCss)
					let finalCss = "";
					if (inlinedCss) {
						const {
							css,
							html: minifiedHtml,
							classMap,
						} = minifyClassNames(inlinedCss, html, sharedClasses);
						html = minifiedHtml;
						finalCss = css;
						totalClassesMinified += classMap.size;
					}
					// Append no-minify CSS as-is
					if (noMinifyCss) {
						finalCss += noMinifyCss;
					}
					// Replace first CSS tag with combined styles
					if (firstCssTag) {
						if (finalCss) {
							html = html.replace(firstCssTag, `<style>${finalCss}</style>`);
						} else {
							html = html.replace(firstCssTag, "");
						}
					}

					// Inject styles.css link at the beginning of head
					const stylesLink = `<link rel="stylesheet" href="${assetsPath}/styles.css">`;
					html = html.replace("<head>", `<head>${stylesLink}`);

					// Inline IIFE
					if (iifeContent.trim().length > 0) {
						if (html.includes("</body>")) {
							html = html.replace(
								"</body>",
								`<script>${iifeContent}</script></body>`,
							);
						} else {
							html = `${html}<script>${iifeContent}</script>`;
						}
					}

					// Replace __APP_ASSETS__ placeholder with app path from config
					const config = JSON.parse(readFileSync("./config.json", "utf-8"));
					html = html.replaceAll("/__APP_ASSETS__/", `${config.assets}/`);
					html = html.replaceAll("/__APP_ASSETS__", `${config.assets}`);

					// Minify HTML
					html = html
						.replace(/<!--[\s\S]*?-->/g, "")
						.replace(/\s+/g, " ")
						.replace(/>\s+</g, "><")
						.replace(/"\s+>/g, '">')
						.replace(/\s+"/g, '"')
						.trim();

					writeFileSync(htmlFile, html, "utf-8");
				}

				// Delete inlined CSS files
				for (const file of filesToDelete) {
					unlinkSync(file);
				}

				console.log(
					`✓ Inlined CSS and IIFE into ${htmlFiles.length} HTML file(s)`,
				);
				if (totalClassesMinified > 0) {
					console.log(`✓ Minified ${totalClassesMinified} CSS class names`);
				}
				if (filesToDelete.size > 0) {
					console.log(`✓ Deleted ${filesToDelete.size} inlined CSS file(s)`);
				}
			} catch (error) {
				console.error("Failed to process build:", error.message);
			}
		},
	};
}
