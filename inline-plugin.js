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
import { build } from "vite";

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
  const markerStartRegex =
    /\.gl-minify-disable-[\w-]+\s*\{\s*--marker:\s*1\s*\}/g;
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

// Cache for compiled IIFEs to avoid recompilation
const iifeCache = new Map();

/**
 * Compile IIFE with specific define values
 * @param {boolean} inlineConfig - Whether to include config fetching
 * @returns {Promise<string>} Compiled IIFE content
 */
async function compileIIFE(inlineConfig) {
  const cacheKey = `config:${inlineConfig}`;
  if (iifeCache.has(cacheKey)) {
    return iifeCache.get(cacheKey);
  }

  const entryPath = join(import.meta.dirname, "web", "inline", "inline.ts");

  const result = await build({
    configFile: false,
    root: import.meta.dirname,
    logLevel: "silent",
    build: {
      write: false,
      lib: {
        formats: ["iife"],
        name: "inline",
        entry: entryPath,
      },
      minify: true,
      rollupOptions: {
        output: {
          // Prevent any external imports
          inlineDynamicImports: true,
        },
      },
    },
    define: {
      "import.meta.env.INLINE_CONFIG": inlineConfig ? "true" : "false",
    },
  });

  // Extract the compiled code from the build result
  const output = result.output || result[0]?.output;
  if (!output || output.length === 0) {
    throw new Error("IIFE compilation produced no output");
  }

  const code = output[0].code;
  iifeCache.set(cacheKey, code);
  return code;
}

/**
 * Scan source HTML files to determine which IIFE variants are needed
 * Looks for <script>"IIFE:config";</script> markers in HTML
 * @param {string} sourceDir - Source directory (e.g., "web/public" or "web/app")
 * @returns {{needsBase: boolean, needsConfig: boolean, fileMarkers: Map<string, string>}}
 */
function scanHtmlForIIFEMarkers(sourceDir) {
  const htmlFiles = globSync(`${sourceDir}/**/*.html`);
  let needsBase = false;
  let needsConfig = false;
  const fileMarkers = new Map(); // Maps filename to marker type

  // Match <script>"IIFE:config";</script> or <script>"IIFE:config"</script>
  const configMarkerRegex = /<script>\s*"IIFE:config"\s*;?\s*<\/script>/;

  for (const htmlFile of htmlFiles) {
    const content = readFileSync(htmlFile, "utf-8");
    const filename = htmlFile.replace(`${sourceDir}/`, "");

    // Check for IIFE:config marker
    if (configMarkerRegex.test(content)) {
      needsConfig = true;
      fileMarkers.set(filename, "config");
    } else {
      needsBase = true;
      fileMarkers.set(filename, "base");
    }
  }

  return { needsBase, needsConfig, fileMarkers };
}

/**
 * Plugin to:
 * 1. Minify and copy styles.css to dist
 * 2. Inline small CSS files (<20KB) into HTML
 * 3. Compile and inline appropriate IIFE script into HTML based on markers
 * 4. Replace asset path placeholders
 * 5. Minify CSS class names
 * 6. Minify HTML
 *
 * @param {Object} options
 * @param {string} options.assetsPath - The base path for assets (e.g., "/login" or "/fiery-sparrow")
 * @param {string} options.sourceDir - The source directory for HTML files (e.g., "web/public" or "web/app")
 */
export function inlineIIFEPlugin(options = {}) {
  const { assetsPath, sourceDir } = options;

  // Scan source HTML files to determine which IIFE variants are needed
  const { needsBase, needsConfig, fileMarkers } = scanHtmlForIIFEMarkers(
    join(import.meta.dirname, sourceDir),
  );

  return {
    name: "inline-iife",
    async closeBundle() {
      const buildType = process.env.BUILD || "login";
      const distDir = join(import.meta.dirname, "dist", buildType);

      try {
        // Compile needed IIFE variants
        let baseIIFE = "";
        let configIIFE = "";

        if (needsBase) {
          baseIIFE = await compileIIFE(false);
          console.log("✓ Compiled base IIFE");
        }
        if (needsConfig) {
          configIIFE = await compileIIFE(true);
          console.log("✓ Compiled config IIFE");
        }

        const htmlFiles = globSync(`${distDir}/*.html`);
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
          const filename = htmlFile.replace(`${distDir}/`, "");
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

          // Select the appropriate IIFE based on the marker for this file
          const markerType = fileMarkers.get(filename) || "base";
          const iifeContent = markerType === "config" ? configIIFE : baseIIFE;

          // Remove any IIFE marker script tags (IIFE:config, IIFE:base, etc.)
          html = html.replace(/<script>\s*"IIFE:[^"]*"\s*;?\s*<\/script>/g, "");

          // Inline IIFE into head (runs early, before body parses)
          if (iifeContent.trim().length > 0) {
            html = html.replace(
              "</head>",
              `<script>${iifeContent}</script></head>`,
            );
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
        throw error;
      }
    },
  };
}
