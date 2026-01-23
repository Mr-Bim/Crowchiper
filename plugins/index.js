import { globSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import {
  processSharedStyles,
  processCssMinification,
  minifyJsClassNames,
} from "./css-minify.js";
import {
  scanHtmlForIIFEMarkers,
  compileNeededIIFEs,
  injectIIFE,
} from "./inline-iife.js";
import { processSri, hashAllJsAssets, writeCspHashes } from "./sri.js";
import {
  stripTestIds,
  minifyHtml,
  collectInlinableCss,
  replaceCssTagWithInline,
  injectSharedStylesLink,
  replaceAssetPlaceholders,
  deleteFiles,
} from "./html-utils.js";

/**
 * Main build plugin that orchestrates all post-build processing in the correct order.
 *
 * Processing order for each HTML file:
 * 1. Collect and inline CSS
 * 2. Minify CSS class names (and update JS files)
 * 3. Inject IIFE script
 * 4. Replace asset placeholders
 * 5. Strip test IDs (when not in test mode)
 * 6. Minify HTML
 * 7. Add SRI attributes
 *
 * After all files:
 * 8. Write CSP hashes to dist/csp-hashes.json
 *
 * @param {Object} options
 * @param {string} options.assetsPath - The base path for assets (e.g., "/login" or "/fiery-sparrow")
 * @param {string} options.sourceDir - The source directory for HTML files (e.g., "web/public" or "web/app")
 * @param {boolean} options.testMode - Whether test mode is enabled (keeps data-testid attributes)
 */
export function buildPlugin(options = {}) {
  const { assetsPath, sourceDir, testMode = false } = options;

  return {
    name: "build-plugin",
    apply: "build",
    async closeBundle() {
      const buildType = process.env.BUILD || "login";
      const rootDir = import.meta.dirname.replace("/plugins", "");
      const distDir = join(rootDir, "dist", buildType);
      const distRoot = join(rootDir, "dist");

      try {
        // Read config for asset path replacement
        const config = JSON.parse(
          readFileSync(join(rootDir, "config.json"), "utf-8"),
        );

        // 1. Process shared styles.css
        const sharedClasses = processSharedStyles(rootDir, distDir);

        // 2. Scan and compile IIFE variants
        const { needsBase, needsConfig, fileMarkers } = scanHtmlForIIFEMarkers(
          join(rootDir, sourceDir),
        );
        const { baseIIFE, configIIFE } = await compileNeededIIFEs(
          rootDir,
          needsBase,
          needsConfig,
        );

        // 3. Process HTML files
        const htmlFiles = globSync(`${distDir}/*.html`);
        const allFilesToDelete = new Set();
        const allScriptHashes = [];
        const allStyleHashes = [];
        let totalClassesMinified = 0;

        for (const htmlFile of htmlFiles) {
          let html = readFileSync(htmlFile, "utf-8");
          const filename = htmlFile.replace(`${distDir}/`, "");

          // 3a. Collect inlinable CSS
          const cssResult = collectInlinableCss(html, assetsPath, distDir);
          html = cssResult.html;
          for (const f of cssResult.filesToDelete) {
            allFilesToDelete.add(f);
          }

          // 3b. Minify CSS class names
          const {
            html: minifiedHtml,
            css,
            classMap,
          } = processCssMinification({
            html,
            inlinedCss: cssResult.inlinedCss,
            filename,
            sharedClasses,
            testMode,
          });
          html = minifiedHtml;
          totalClassesMinified += classMap.size;

          // 3c. Apply class map to JS files (only once per unique classMap)
          if (classMap.size > 0) {
            const jsFiles = globSync(`${distDir}/**/*.js`);
            for (const jsFile of jsFiles) {
              const jsContent = readFileSync(jsFile, "utf-8");
              const minifiedJs = minifyJsClassNames(jsContent, classMap);
              if (jsContent !== minifiedJs) {
                writeFileSync(jsFile, minifiedJs);
              }
            }
          }

          // 3d. Replace CSS tag with inlined styles
          html = replaceCssTagWithInline(html, cssResult.firstCssTag, css);

          // 3e. Inject shared styles link
          html = injectSharedStylesLink(html, assetsPath);

          // 3f. Inject IIFE
          const markerType = fileMarkers.get(filename) || "base";
          const iifeContent = markerType === "config" ? configIIFE : baseIIFE;
          html = injectIIFE(html, iifeContent);

          // 3g. Replace asset placeholders
          html = replaceAssetPlaceholders(html, config.assets);

          // 3h. Strip test IDs when not in test mode
          if (!testMode) {
            html = stripTestIds(html);
          }

          // 3i. Minify HTML
          html = minifyHtml(html);

          // 3j. Add SRI attributes (must be last as it hashes final content)
          const sriResult = processSri(html, assetsPath, distDir);
          html = sriResult.html;
          allScriptHashes.push(...sriResult.scriptHashes);
          allStyleHashes.push(...sriResult.styleHashes);

          // Write final HTML
          writeFileSync(htmlFile, html, "utf-8");
        }

        // 4. Delete inlined CSS files
        deleteFiles([...allFilesToDelete]);

        // 5. Hash all JS assets (including dynamically imported chunks)
        const jsAssetHashes = hashAllJsAssets(distDir);
        allScriptHashes.push(...jsAssetHashes);

        // 6. Write CSP hashes
        // Load existing hashes or create new object
        const cspHashesPath = join(distRoot, "csp-hashes.json");
        let existingHashes = {};
        try {
          existingHashes = JSON.parse(readFileSync(cspHashesPath, "utf-8"));
        } catch {
          // File doesn't exist yet, start fresh
        }

        // Deduplicate hashes and store separately for scripts and styles
        const uniqueScriptHashes = [...new Set(allScriptHashes)];
        const uniqueStyleHashes = [...new Set(allStyleHashes)];
        existingHashes[buildType] = {
          scripts: uniqueScriptHashes,
          styles: uniqueStyleHashes,
        };
        writeCspHashes(distRoot, existingHashes);

        // Log summary
        console.log(`✓ Processed ${htmlFiles.length} HTML file(s)`);
        if (totalClassesMinified > 0) {
          console.log(`✓ Minified ${totalClassesMinified} CSS class names`);
        }
        if (allFilesToDelete.size > 0) {
          console.log(`✓ Deleted ${allFilesToDelete.size} inlined CSS file(s)`);
        }
        console.log(
          `✓ Collected ${uniqueScriptHashes.length} script hashes and ${uniqueStyleHashes.length} style hashes for ${buildType}`,
        );
      } catch (error) {
        console.error("Failed to process build:", error.message);
        throw error;
      }
    },
  };
}
