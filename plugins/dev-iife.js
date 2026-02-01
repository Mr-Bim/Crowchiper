import { join } from "node:path";
import { build } from "vite";

// Cache for compiled IIFEs during dev
let cachedBaseIIFE = null;
let cachedConfigIIFE = null;

/**
 * Compile IIFE for dev server (unminified for easier debugging)
 * @param {string} rootDir - Project root directory
 * @param {boolean} inlineConfig - Whether to include config fetching
 * @returns {Promise<string>} Compiled IIFE content
 */
async function compileIIFEForDev(rootDir, inlineConfig) {
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
      minify: false, // Keep unminified for dev
      rollupOptions: {
        output: {
          inlineDynamicImports: true,
        },
      },
    },
    define: {
      "import.meta.env.INLINE_CONFIG": inlineConfig ? "true" : "false",
    },
  });

  const output = result.output || result[0]?.output;
  if (!output || output.length === 0) {
    throw new Error("IIFE compilation produced no output");
  }

  return output[0].code;
}

/**
 * Dev server plugin that injects IIFE into HTML during development.
 * Uses transformIndexHtml hook which works with Vite's dev server.
 *
 * @param {Object} options
 * @param {string} options.assetsPath - The base path for assets
 */
export function devIifePlugin(options = {}) {
  const { assetsPath } = options;
  let rootDir;

  return {
    name: "dev-iife-plugin",
    apply: "serve",

    configResolved(config) {
      // Get the project root (parent of web/login or web/app)
      rootDir = config.root.replace(/\/web\/(login|app)$/, "");
    },

    async transformIndexHtml(html) {
      // Compile IIFE if not cached
      if (needsConfig && !cachedConfigIIFE) {
        cachedConfigIIFE = await compileIIFEForDev(rootDir, true);
        console.log("✓ Compiled config IIFE for dev");
      }
      if (!needsConfig && !cachedBaseIIFE) {
        cachedBaseIIFE = await compileIIFEForDev(rootDir, false);
        console.log("✓ Compiled base IIFE for dev");
      }

      const iifeContent = needsConfig ? cachedConfigIIFE : cachedBaseIIFE;

      // Remove IIFE marker
      let result = html.replace(
        /<script>\s*"IIFE:[^"]*"\s*;?\s*<\/script>/g,
        "",
      );

      // Replace __APP_ASSETS__ placeholder in the IIFE content
      const processedIife = iifeContent.replace(/__APP_ASSETS__/g, assetsPath);

      // Inject IIFE into head
      result = result.replace(
        "</head>",
        `<script>${processedIife}</script></head>`,
      );

      return result;
    },
  };
}

/**
 * Clear IIFE cache (useful for HMR of inline.ts)
 */
export function clearIifeCache() {
  cachedBaseIIFE = null;
  cachedConfigIIFE = null;
}
