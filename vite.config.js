// vite.config.js
import { globSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import { inlineIIFEPlugin } from "./inline-plugin.js";
import { sriPlugin } from "./sri-plugin.js";

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
  plugins: [
    inlineIIFEPlugin({ assetsPath: "/login", sourceDir: "web/public" }),
    sriPlugin({ assetsPath: "/login" }),
  ],
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
    // Test mode is disabled by default.
    // Set TEST_MODE=1 to enable test features for development/testing.
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
  plugins: [
    inlineIIFEPlugin({ assetsPath: config.assets, sourceDir: "web/app" }),
    sriPlugin({ assetsPath: config.assets }),
  ],
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

// Select build based on environment variable
let out;
if (process.env.BUILD === "login") {
  out = login;
} else if (process.env.BUILD === "app") {
  out = app;
} else {
  // Default to login build
  out = login;
}

export default out;
