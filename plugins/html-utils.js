import { existsSync, readFileSync, statSync, unlinkSync } from "node:fs";
import { join } from "node:path";

/**
 * Strip all data-testid attributes from HTML content
 * @param {string} html - HTML content
 * @returns {string} HTML with data-testid attributes removed
 */
export function stripTestIds(html) {
  return html.replace(/\s*data-testid="[^"]*"/g, "");
}

/**
 * Minify HTML by removing whitespace and comments
 * @param {string} html - HTML content
 * @returns {string} Minified HTML
 */
export function minifyHtml(html) {
  return html
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\s+/g, " ")
    .replace(/>\s+</g, "><")
    .replace(/"\s+>/g, '">')
    .replace(/\s+"/g, '"')
    .trim();
}

/**
 * Find and collect CSS files that should be inlined (under 20KB)
 * @param {string} html - HTML content
 * @param {string} assetsPath - Base path for assets
 * @param {string} distDir - Distribution directory
 * @returns {{inlinedCss: string, firstCssTag: string|null, tagsToRemove: string[], filesToDelete: string[], html: string}}
 */
export function collectInlinableCss(html, assetsPath, distDir) {
  const assetsPattern = assetsPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const linkRegex =
    /<link[^>]*(?:rel="stylesheet"[^>]*href="([^"]+)"|href="([^"]+)"[^>]*rel="stylesheet")[^>]*>/g;

  let inlinedCss = "";
  let firstCssTag = null;
  const filesToDelete = [];
  let resultHtml = html;

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
        inlinedCss += cssContent;

        // Track first CSS tag for replacement, remove others
        if (!firstCssTag) {
          firstCssTag = fullTag;
        } else {
          resultHtml = resultHtml.replace(fullTag, "");
        }
        filesToDelete.push(cssFilePath);
      }
    }
  }

  return { inlinedCss, firstCssTag, filesToDelete, html: resultHtml };
}

/**
 * Replace first CSS tag with inlined styles
 * @param {string} html - HTML content
 * @param {string|null} firstCssTag - The first CSS link tag to replace
 * @param {string} css - CSS content to inline
 * @returns {string} Updated HTML
 */
export function replaceCssTagWithInline(html, firstCssTag, css) {
  if (!firstCssTag) return html;

  if (css) {
    return html.replace(firstCssTag, `<style>${css}</style>`);
  }
  return html.replace(firstCssTag, "");
}

/**
 * Inject shared styles.css link at beginning of head
 * @param {string} html - HTML content
 * @param {string} assetsPath - Base path for assets
 * @param {string} stylesFilename - Hashed styles filename (e.g., "styles-abc12345.css")
 * @returns {string} Updated HTML
 */
export function injectSharedStylesLink(html, assetsPath, stylesFilename) {
  if (!stylesFilename) return html;
  const stylesLink = `<link rel="stylesheet" href="${assetsPath}/assets/${stylesFilename}">`;
  return html.replace("<head>", `<head>${stylesLink}`);
}

/**
 * Replace asset path placeholders in HTML
 * @param {string} html - HTML content
 * @param {string} assetsPath - Actual assets path
 * @returns {string} Updated HTML
 */
export function replaceAssetPlaceholders(html, assetsPath) {
  return html
    .replaceAll("/__APP_ASSETS__/", `${assetsPath}/`)
    .replaceAll("/__APP_ASSETS__", `${assetsPath}`);
}

/**
 * Delete files from filesystem
 * @param {string[]} files - Array of file paths to delete
 */
export function deleteFiles(files) {
  for (const file of files) {
    unlinkSync(file);
  }
}
