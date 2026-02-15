import { globSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { build } from "vite";

// Cache for compiled IIFEs to avoid recompilation
const iifeCache = new Map();

/**
 * Compile IIFE with specific define values
 * @param {string} rootDir - Project root directory
 * @param {boolean} inlineConfig - Whether to include config fetching
 * @returns {Promise<string>} Compiled IIFE content
 */
export async function compileIIFE(rootDir, inlineConfig) {
  const cacheKey = `config:${inlineConfig}`;
  if (iifeCache.has(cacheKey)) {
    return iifeCache.get(cacheKey);
  }

  const entryPath = join(rootDir, "web", "inline", "inline.ts");

  const result = await build({
    configFile: false,
    root: rootDir,
    logLevel: "silent",
    build: {
      write: false,
      lib: {
        formats: ["iife"],
        name: "inline",
        entry: entryPath,
      },
      minify: true,
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
export function scanHtmlForIIFEMarkers(sourceDir) {
  const htmlFiles = globSync(`${sourceDir}/**/*.html`);
  let needsBase = false;
  let needsConfig = false;
  const fileMarkers = new Map();

  // Match <script>"IIFE:config";</script> or <script>"IIFE:config"</script>
  const configMarkerRegex = /<script>\s*"IIFE:config"\s*;?\s*<\/script>/;

  for (const htmlFile of htmlFiles) {
    const content = readFileSync(htmlFile, "utf-8");
    const filename = htmlFile.replace(`${sourceDir}/`, "");

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
 * Compile needed IIFE variants based on markers
 * @param {string} rootDir - Project root directory
 * @param {boolean} needsBase - Whether base IIFE is needed
 * @param {boolean} needsConfig - Whether config IIFE is needed
 * @returns {Promise<{baseIIFE: string, configIIFE: string}>}
 */
export async function compileNeededIIFEs(rootDir, needsBase, needsConfig) {
  let baseIIFE = "";
  let configIIFE = "";

  if (needsBase) {
    baseIIFE = await compileIIFE(rootDir, false);
    console.log("✓ Compiled base IIFE");
  }
  if (needsConfig) {
    configIIFE = await compileIIFE(rootDir, true);
    console.log("✓ Compiled config IIFE");
  }

  return { baseIIFE, configIIFE };
}

/**
 * Inject IIFE into HTML head
 * @param {string} html - HTML content
 * @param {string} iifeContent - IIFE JavaScript content
 * @returns {string} HTML with IIFE injected
 */
export function injectIIFE(html, iifeContent) {
  // Remove any IIFE marker script tags (IIFE:config, IIFE:base, etc.)
  let result = html.replace(/<script>\s*"IIFE:[^"]*"\s*;?\s*<\/script>/g, "");

  // Inline IIFE into head (runs early, before body parses)
  if (iifeContent.trim().length > 0) {
    result = result.replace(
      "</head>",
      `<script>${iifeContent}</script></head>`,
    );
  }

  return result;
}
