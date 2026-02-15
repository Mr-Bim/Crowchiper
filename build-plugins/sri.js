import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { globSync } from "node:fs";

/**
 * Generate SHA-384 integrity hash for content
 * @param {string|Buffer} content - Content to hash
 * @returns {string} SRI hash in format "sha384-..."
 */
export function getIntegrity(content) {
  return "sha384-" + createHash("sha384").update(content).digest("base64");
}

/**
 * Resolve a URL to a file path and compute its integrity hash
 * @param {string} url - URL to resolve
 * @param {string} assetsPath - Base path for assets
 * @param {string} distDir - Distribution directory
 * @returns {{filePath: string, integrity: string} | null} File path and integrity, or null if not local
 */
function resolveAndHash(url, assetsPath, distDir) {
  if (!url.startsWith(assetsPath) && !url.startsWith("/")) {
    return null;
  }

  let relativePath = url;
  if (url.startsWith(assetsPath)) {
    relativePath = url.slice(assetsPath.length);
  }

  // Remove leading slash if present to join correctly
  if (relativePath.startsWith("/")) {
    relativePath = relativePath.slice(1);
  }

  const filePath = join(distDir, relativePath);

  try {
    const content = readFileSync(filePath);
    const integrity = getIntegrity(content);
    return { filePath, integrity };
  } catch (e) {
    console.warn(`Could not read file for SRI: ${filePath} (${e.message})`);
    return null;
  }
}

/**
 * Add SRI attributes to external script tags
 * @param {string} html - HTML content
 * @param {string} assetsPath - Base path for assets
 * @param {string} distDir - Distribution directory
 * @returns {{html: string, hashes: string[]}} Updated HTML and collected hashes
 */
export function addSriToExternalScripts(html, assetsPath, distDir) {
  const hashes = [];

  const result = html.replace(
    /<script([^>]*)src="([^"]+)"([^>]*)><\/script>/g,
    (match, beforeSrc, src, afterSrc) => {
      const resolved = resolveAndHash(src, assetsPath, distDir);
      if (resolved) {
        hashes.push(resolved.integrity);
        return `<script${beforeSrc}src="${src}" integrity="${resolved.integrity}"${afterSrc}></script>`;
      }
      return match;
    },
  );

  return { html: result, hashes };
}

/**
 * Add SRI attributes to modulepreload links and collect hashes for CSP
 * @param {string} html - HTML content
 * @param {string} assetsPath - Base path for assets
 * @param {string} distDir - Distribution directory
 * @returns {{html: string, hashes: string[]}} Updated HTML and collected hashes
 */
export function addSriToModulePreloads(html, assetsPath, distDir) {
  const hashes = [];

  const result = html.replace(
    /<link([^>]*)rel="modulepreload"([^>]*)>/g,
    (match, before, after) => {
      // Extract href from the tag
      const fullAttrs = before + after;
      const hrefMatch = fullAttrs.match(/href="([^"]+)"/);
      if (!hrefMatch) {
        return match;
      }

      const href = hrefMatch[1];
      const resolved = resolveAndHash(href, assetsPath, distDir);
      if (resolved) {
        hashes.push(resolved.integrity);
        // Add integrity attribute if not already present
        if (!match.includes("integrity=")) {
          return match.replace(">", ` integrity="${resolved.integrity}">`);
        }
      }
      return match;
    },
  );

  return { html: result, hashes };
}

/**
 * Add SRI attributes to inline script tags
 * @param {string} html - HTML content
 * @returns {{html: string, hashes: string[]}} Updated HTML and collected hashes
 */
export function addSriToInlineScripts(html) {
  const hashes = [];

  const result = html.replace(
    /<script([^>]*)>([\s\S]*?)<\/script>/g,
    (match, attrs, content) => {
      if (attrs.includes('src="')) {
        return match;
      }
      if (!content || !content.trim()) {
        return match;
      }

      const integrity = getIntegrity(content);
      hashes.push(integrity);
      return `<script${attrs} integrity="${integrity}">${content}</script>`;
    },
  );

  return { html: result, hashes };
}

/**
 * Hash inline style tags for CSP
 * @param {string} html - HTML content
 * @returns {{hashes: string[]}} Collected style hashes
 */
export function hashInlineStyles(html) {
  const hashes = [];

  html.replace(
    /<style([^>]*)>([\s\S]*?)<\/style>/g,
    (match, attrs, content) => {
      if (!content || !content.trim()) {
        return match;
      }
      const integrity = getIntegrity(content);
      hashes.push(integrity);
      return match;
    },
  );

  return { hashes };
}

/**
 * Process HTML file and add SRI to all scripts
 * @param {string} html - HTML content
 * @param {string} assetsPath - Base path for assets
 * @param {string} distDir - Distribution directory
 * @returns {{html: string, scriptHashes: string[], styleHashes: string[]}} Updated HTML and collected hashes
 */
export function processSri(html, assetsPath, distDir) {
  const scriptHashes = [];

  // Process external scripts
  const external = addSriToExternalScripts(html, assetsPath, distDir);
  scriptHashes.push(...external.hashes);

  // Process modulepreload links (these are also scripts that need CSP hashes)
  const modulePreloads = addSriToModulePreloads(
    external.html,
    assetsPath,
    distDir,
  );
  scriptHashes.push(...modulePreloads.hashes);

  // Process inline scripts
  const inline = addSriToInlineScripts(modulePreloads.html);
  scriptHashes.push(...inline.hashes);

  // Hash inline styles (no modification needed, just collect hashes)
  const styles = hashInlineStyles(inline.html);

  return { html: inline.html, scriptHashes, styleHashes: styles.hashes };
}

/**
 * Hash all JS files in the assets directory for dynamic imports
 * @param {string} distDir - Distribution directory
 * @returns {string[]} Array of integrity hashes
 */
export function hashAllJsAssets(distDir) {
  const hashes = [];
  const jsFiles = globSync(`${distDir}/assets/*.js`);

  for (const filePath of jsFiles) {
    try {
      const content = readFileSync(filePath);
      const integrity = getIntegrity(content);
      if (filePath.includes("drag-and-drop")) {
        console.log(filePath);
        console.log(integrity);
      }
      hashes.push(integrity);
    } catch (e) {
      console.warn(
        `Could not read JS file for CSP: ${filePath} (${e.message})`,
      );
    }
  }

  return hashes;
}

/**
 * Write CSP hashes to a JSON file
 * @param {string} distDir - Distribution directory (root dist folder)
 * @param {Object} hashes - Object mapping build types to their hashes
 */
export function writeCspHashes(distDir, hashes) {
  const cspPath = join(distDir, "csp-hashes.json");
  writeFileSync(cspPath, JSON.stringify(hashes, null, 2));
  console.log(`✓ Wrote CSP hashes to ${cspPath}`);
}
