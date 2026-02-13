// vite.config.js
import { globSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import { buildPlugin } from "./build-plugins/index.js";
import { devServerPlugin } from "./build-plugins/dev-server.js";

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

// Default Rust server port for API proxy
const RUST_SERVER_PORT = process.env.RUST_PORT || 7291;

// App definitions for dev server
// Add new apps here to serve them from the unified dev server
const devApps = [
  {
    name: "login",
    base: "/login",
    srcDir: "web/login",
    iifeConfig: true, // Login pages need config IIFE for redirect check
  },
  {
    name: "app",
    base: config.assets,
    srcDir: "web/app",
    iifeConfig: false,
  },
  {
    name: "dashboard",
    base: "/dashboard",
    srcDir: "web/dashboard",
    iifeConfig: false,
  },
];

// Unified dev server configuration
const dev = defineConfig({
  root: ".",
  server: {
    port: 5173,
  },
  define: {
    __TEST_MODE__: JSON.stringify(!!process.env.TEST_MODE),
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
    devServerPlugin({
      apps: devApps,
      assetsPath: config.assets,
      rustPort: RUST_SERVER_PORT,
    }),
  ],
});

// Login build: web/login/ -> dist/login/ with base /login
const loginHtmlFiles = globSync("web/login/**/*.html");
const loginInput = Object.fromEntries(
  loginHtmlFiles.map((file) => {
    const name = file.replace("web/login/", "").replace(".html", "");
    return [name, resolve(__dirname, file)];
  }),
);

const login = defineConfig({
  root: "web/login/",
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
  plugins: [buildPlugin({ assetsPath: "/login", sourceDir: "web/login" })],
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
    buildPlugin({
      assetsPath: config.assets,
      sourceDir: "web/app",
      testMode: !!process.env.TEST_MODE,
    }),
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

// Dashboard build: web/dashboard/ -> dist/dashboard/ with base /dashboard
const dashboardHtmlFiles = globSync("web/dashboard/**/*.html");
const dashboardInput = Object.fromEntries(
  dashboardHtmlFiles.map((file) => {
    const name = file.replace("web/dashboard/", "").replace(".html", "");
    return [name, resolve(__dirname, file)];
  }),
);

const dashboard = defineConfig({
  root: "web/dashboard/",
  base: "/dashboard/",
  build: {
    outDir: "../../dist/dashboard",
    emptyOutDir: true,
    rollupOptions: {
      input: dashboardInput,
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
    buildPlugin({
      assetsPath: "/dashboard",
      sourceDir: "web/dashboard",
      testMode: !!process.env.TEST_MODE,
    }),
  ],
});

// Select config based on BUILD environment variable
// - BUILD=login: Production build for login pages
// - BUILD=app: Production build for app pages
// - BUILD=dashboard: Production build for dashboard pages
// - No BUILD (dev): Unified dev server serving all apps
let out;
if (process.env.BUILD === "login") {
  out = login;
} else if (process.env.BUILD === "app") {
  out = app;
} else if (process.env.BUILD === "dashboard") {
  out = dashboard;
} else {
  out = dev;
}

export default out;
