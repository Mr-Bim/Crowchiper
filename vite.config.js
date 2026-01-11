// vite.config.js
import { globSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { defineConfig } from "vite";
import { inlineIIFEPlugin } from "./inline-plugin.js";

const config = JSON.parse(readFileSync("./config.json", "utf-8"));

// Validate assets path format
if (!config.assets.startsWith("/")) {
  throw new Error(
    `config.json: assets must start with '/', got: ${config.assets}`,
  );
}
if (config.assets.length > 1 && config.assets.endsWith("/")) {
  throw new Error(
    `config.json: assets must not end with '/', got: ${config.assets}`,
  );
}

// Login build: web/public/ -> dist/login/ with base /login
const loginHtmlFiles = globSync("web/public/**/*.html");
const loginInput = Object.fromEntries(
  loginHtmlFiles.map((file) => {
    const name = file.replace("web/public/", "").replace(".html", "");
    return [name, resolve(__dirname, file)];
  }),
);

const login = defineConfig({
  root: "web/public/",
  base: "/login/",
  build: {
    outDir: "../../dist/login",
    emptyOutDir: true,
    rollupOptions: {
      input: loginInput,
    },
    minify: true,
    cssMinify: "lightningcss",
  },
  css: {
    transformer: "lightningcss",
    lightningcss: {
      drafts: {
        customMedia: true,
      },
    },
  },
  plugins: [inlineIIFEPlugin({ assetsPath: "/login" })],
});

// App build: web/app/ -> dist/app/ with base from config.assets
const appHtmlFiles = globSync("web/app/**/*.html");
const appInput = Object.fromEntries(
  appHtmlFiles.map((file) => {
    const name = file.replace("web/app/", "").replace(".html", "");
    return [name, resolve(__dirname, file)];
  }),
);

const app = defineConfig({
  root: "web/app/",
  base: `${config.assets}/`,
  define: {
    // Enable test mode when TEST_MODE env var is set.
    // This allows injecting encryption keys for testing without PRF support.
    __TEST_MODE__: JSON.stringify(!!process.env.TEST_MODE),
  },
  build: {
    outDir: "../../dist/app",
    emptyOutDir: true,
    rollupOptions: {
      input: appInput,
    },
    minify: true,
    cssMinify: "lightningcss",
  },
  css: {
    transformer: "lightningcss",
    lightningcss: {
      drafts: {
        customMedia: true,
      },
    },
  },
  plugins: [inlineIIFEPlugin({ assetsPath: config.assets })],
  experimental: {
    renderBuiltUrl(filename, { hostType }) {
      if (hostType === "js") {
        return {
          runtime: `window.__assetsPath(${JSON.stringify(filename)})`,
        };
      }
      // CSS and HTML use relative paths
      return { relative: true };
    },
  },
});

// IIFE build: shared inline script
const iife = defineConfig({
  root: "web/iife",
  build: {
    outDir: "../../dist/iife",
    emptyOutDir: true,
    lib: {
      formats: ["iife"],
      name: "inline",
      entry: [join(import.meta.dirname, "web", "inline", "inline.ts")],
      fileName: () => "inline.js",
    },
    minify: true,
  },
});

// Select build based on environment variable
let out;
if (process.env.IIFE) {
  out = iife;
} else if (process.env.BUILD === "login") {
  out = login;
} else if (process.env.BUILD === "app") {
  out = app;
} else {
  // Default to login build
  out = login;
}

export default out;
