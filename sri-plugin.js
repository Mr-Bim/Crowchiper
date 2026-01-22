import { globSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";

function getIntegrity(content) {
  return "sha384-" + createHash("sha384").update(content).digest("base64");
}

/**
 * Plugin to add Subresource Integrity (SRI) attributes to script tags.
 * Handles both external scripts (bundled by Vite) and inline scripts.
 *
 * @param {Object} options
 * @param {string} options.assetsPath - The base path for assets
 */
export function sriPlugin(options = {}) {
  const { assetsPath } = options;

  return {
    name: "sri-plugin",
    apply: "build",
    async closeBundle() {
      const buildType = process.env.BUILD || "login";
      const distDir = join(import.meta.dirname, "dist", buildType);

      console.log("Starting SRI injection...");

      try {
        const htmlFiles = globSync(`${distDir}/**/*.html`);

        for (const htmlFile of htmlFiles) {
          let html = readFileSync(htmlFile, "utf-8");
          let modified = false;

          // 1. Process external scripts: <script ... src="...">
          // We use a regex to find script tags with a src attribute
          html = html.replace(
            /<script([^>]*)src="([^"]+)"([^>]*)><\/script>/g,
            (match, beforeSrc, src, afterSrc) => {
              // Check if the script is local (starts with assetsPath)
              if (src.startsWith(assetsPath) || src.startsWith("/")) {
                // Strip assetsPath from the start of src to get relative path in dist.

                let relativePath = src;
                if (src.startsWith(assetsPath)) {
                  relativePath = src.slice(assetsPath.length);
                } else if (assetsPath === "/" && src.startsWith("/")) {
                  relativePath = src;
                }

                // Remove leading slash if present to join correctly
                if (relativePath.startsWith("/")) {
                  relativePath = relativePath.slice(1);
                }

                const filePath = join(distDir, relativePath);

                try {
                  const content = readFileSync(filePath);
                  const integrity = getIntegrity(content);
                  modified = true;
                  // Reconstruct tag with integrity
                  return `<script${beforeSrc}src="${src}" integrity="${integrity}"${afterSrc}></script>`;
                } catch (e) {
                  console.warn(
                    `Could not read file for SRI: ${filePath} (${e.message})`,
                  );
                  return match;
                }
              }
              return match;
            },
          );

          // 2. Process inline scripts: <script>...</script>
          // We exclude tags that have a src attribute
          html = html.replace(
            /<script([^>]*)>([\s\S]*?)<\/script>/g,
            (match, attrs, content) => {
              if (attrs.includes('src="')) {
                return match;
              }
              if (!content || !content.trim()) {
                return match;
              }

              const integrity = getIntegrity(content);
              modified = true;
              return `<script${attrs} integrity="${integrity}">${content}</script>`;
            },
          );

          if (modified) {
            writeFileSync(htmlFile, html, "utf-8");
            console.log(`✓ Added SRI to ${htmlFile.replace(distDir, "")}`);
          }
        }
      } catch (error) {
        console.error("Failed to add SRI:", error);
        throw error;
      }
    },
  };
}
