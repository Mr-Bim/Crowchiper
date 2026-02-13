import { readFileSync, existsSync } from "node:fs";
import { join, extname } from "node:path";
import { build } from "vite";

/**
 * Multi-app dev server plugin.
 *
 * Serves multiple "apps" from different source directories under different base paths.
 * Each app has its own base path (e.g., /login, /fiery-sparrow) and source directory.
 *
 * @param {Object} options
 * @param {Array<{name: string, base: string, srcDir: string, iifeConfig?: boolean}>} options.apps
 * @param {string} options.assetsPath - The app assets path (for IIFE injection)
 * @param {number} options.rustPort - Port where Rust server runs for API proxy
 */
export function devServerPlugin(options) {
  const { apps, assetsPath, rustPort = 7291 } = options;

  // Build a map of base path -> app config
  const appsByBase = new Map();
  for (const app of apps) {
    // Normalize base to not have trailing slash
    const normalizedBase = app.base.endsWith("/")
      ? app.base.slice(0, -1)
      : app.base;
    appsByBase.set(normalizedBase, app);
  }

  // Cache for compiled IIFEs
  const iifeCache = new Map();

  /**
   * Compile IIFE for dev (unminified)
   */
  async function compileIIFE(rootDir, inlineConfig) {
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
        minify: false,
      },
      define: {
        "import.meta.env.INLINE_CONFIG": inlineConfig ? "true" : "false",
      },
    });

    const output = result.output || result[0]?.output;
    if (!output || output.length === 0) {
      throw new Error("IIFE compilation produced no output");
    }

    const code = output[0].code;
    iifeCache.set(cacheKey, code);
    return code;
  }

  /**
   * Rewrite relative paths in HTML to absolute paths pointing to source directory.
   * This is needed because Vite's transformIndexHtml resolves paths against the URL,
   * not the actual file location.
   */
  function rewriteRelativePaths(html, srcDir) {
    // Rewrite src="..." and href="..." that are relative (not starting with / or http)
    return html.replace(
      /(src|href)="(?!\/|https?:\/\/|#)([^"]+)"/g,
      (match, attr, path) => {
        // Convert relative path to absolute path pointing to source
        return `${attr}="/${srcDir}/${path}"`;
      },
    );
  }

  /**
   * Inject IIFE and shared styles into HTML content
   */
  function injectIIFE(html, iifeContent) {
    // Remove IIFE marker
    let result = html.replace(/<script>\s*"IIFE:[^"]*"\s*;?\s*<\/script>/g, "");

    // Replace __APP_ASSETS__ placeholder
    const processedIife = iifeContent.replace(/__APP_ASSETS__/g, assetsPath);

    // Inject shared styles.css link at beginning of head
    result = result.replace(
      "<head>",
      `<head><link rel="stylesheet" href="/web/styles.css">`,
    );

    // Inject IIFE into end of head
    result = result.replace(
      "</head>",
      `<script>${processedIife}</script></head>`,
    );

    return result;
  }

  let rootDir;

  return {
    name: "multi-app-dev-server",
    apply: "serve",

    config() {
      return {
        server: {
          proxy: {
            "/api": {
              target: `http://127.0.0.1:${rustPort}`,
              changeOrigin: true,
            },
          },
        },
      };
    },

    configResolved(config) {
      rootDir = config.root;
    },

    handleHotUpdate({ file, server }) {
      // Clear IIFE cache when inline.ts changes
      if (file.endsWith("inline/inline.ts")) {
        iifeCache.clear();
        console.log("✓ Cleared IIFE cache (inline.ts changed)");
      }

      // Trigger full page reload for all file changes in web/
      if (file.includes("/web/")) {
        server.ws.send({ type: "full-reload" });
        return []; // Prevent default HMR handling
      }
    },

    configureServer(server) {
      // Add middleware to handle requests for different apps
      server.middlewares.use(async (req, res, next) => {
        const url = req.url || "/";
        const pathname = url.split("?")[0];

        // Redirect root to /login
        if (pathname === "/" || pathname === "") {
          res.writeHead(302, { Location: "/login" });
          res.end();
          return;
        }

        // Serve shared styles.css from web/styles.css
        if (pathname === "/styles.css" || pathname === "/web/styles.css") {
          const stylesPath = join(rootDir, "web", "styles.css");
          if (existsSync(stylesPath)) {
            req.url = "/web/styles.css";
            return next();
          }
        }

        // Find which app this request belongs to
        let matchedApp = null;
        let matchedBase = "";

        for (const [base, app] of appsByBase) {
          if (pathname === base || pathname.startsWith(base + "/")) {
            matchedApp = app;
            matchedBase = base;
            break;
          }
        }

        if (!matchedApp) {
          // No app matched, let Vite handle it (might be HMR, etc.)
          return next();
        }

        // Calculate the path relative to the app's base
        let relativePath = pathname.slice(matchedBase.length) || "/";
        if (relativePath === "/") {
          relativePath = "/index.html";
        }

        // Check if this is an HTML request
        const isHtmlRequest =
          relativePath.endsWith(".html") ||
          (!extname(relativePath) && req.headers.accept?.includes("text/html"));

        if (isHtmlRequest) {
          // Ensure .html extension
          if (!relativePath.endsWith(".html")) {
            relativePath = relativePath + ".html";
          }

          const htmlPath = join(rootDir, matchedApp.srcDir, relativePath);

          if (!existsSync(htmlPath)) {
            return next();
          }

          try {
            let html = readFileSync(htmlPath, "utf-8");

            // Rewrite relative paths to absolute paths before Vite processes them
            html = rewriteRelativePaths(html, matchedApp.srcDir);

            // Transform HTML through Vite's pipeline
            html = await server.transformIndexHtml(url, html);

            // Check if needs config IIFE
            const configMarkerRegex =
              /<script>\s*"IIFE:config"\s*;?\s*<\/script>/;
            const needsConfig = configMarkerRegex.test(html);

            // Compile and inject IIFE
            const iifeContent = await compileIIFE(
              rootDir,
              matchedApp.iifeConfig ?? needsConfig,
            );
            html = injectIIFE(html, iifeContent);

            res.setHeader("Content-Type", "text/html");
            res.end(html);
            return;
          } catch (err) {
            console.error(`Error serving ${htmlPath}:`, err);
            return next(err);
          }
        }

        // For non-HTML requests (CSS, JS, images), rewrite the URL to point to source
        const srcPath = join(matchedApp.srcDir, relativePath);
        req.url =
          "/" +
          srcPath +
          (url.includes("?") ? url.slice(url.indexOf("?")) : "");
        return next();
      });
    },
  };
}
