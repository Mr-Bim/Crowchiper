// Calculate BASE_PATH from current URL by removing /login or app assets path
(() => {
  const path = window.location.pathname;
  let base = "";

  const assets = "__APP_ASSETS__";
  // If we're on a /login page, remove /login and everything after
  if (path.includes("/login")) {
    const loginIndex = path.indexOf("/login");
    base = path.substring(0, loginIndex);
  }
  // If we're on a /dashboard page, remove /dashboard and everything after
  else if (path.includes("/dashboard")) {
    const dashboardIndex = path.indexOf("/dashboard");
    base = path.substring(0, dashboardIndex);
  }
  // Otherwise, remove /__APP_ASSETS__ and everything after (will be replaced by build)
  else if (path.includes(assets)) {
    const appIndex = path.indexOf(assets);
    base = path.substring(0, appIndex);
  }

  const appPath = `${base}${assets}`;

  // Define path constants as non-writable/non-configurable to prevent tampering
  Object.defineProperty(window, "API_PATH", {
    value: `${base}/api`,
    writable: false,
    configurable: false,
  });
  Object.defineProperty(window, "LOGIN_PATH", {
    value: `${base}/login`,
    writable: false,
    configurable: false,
  });
  Object.defineProperty(window, "APP_PATH", {
    value: appPath,
    writable: false,
    configurable: false,
  });

  // Provide __assetsPath for Vite's renderBuiltUrl runtime resolution
  // This allows dynamic imports to resolve correctly with runtime base path
  Object.defineProperty(window, "__assetsPath", {
    value: (filename: string) => `${base}${assets}/${filename}`,
    writable: false,
    configurable: false,
  });

  // Conditionally fetch config at startup (for login/register pages)
  // Redirect immediately if already authenticated
  if (import.meta.env.INLINE_CONFIG) {
    interface ServerConfig {
      no_signup: boolean;
      authenticated: boolean;
    }

    const configPromise: Promise<ServerConfig> = fetch(`${base}/api/config`)
      .then((response) => {
        if (response.ok) {
          return response.json();
        }
        return { no_signup: false, authenticated: false };
      })
      .then((config: ServerConfig) => {
        // Redirect immediately if authenticated
        if (config.authenticated) {
          window.location.href = appPath;
        }
        return config;
      })
      .catch(() => {
        return { no_signup: false, authenticated: false };
      });

    (
      window as unknown as Record<string, Promise<ServerConfig>>
    ).__CONFIG_PROMISE__ = configPromise;
  }
})();

import { getStorage, setStorage } from "../shared/storage.ts";

// Theme switching logic
const themes = [
  { id: "warm-light", label: "🔥 Ember" },
  { id: "scandi-dark", label: "🪨 Slate" },
  { id: "paper-light", label: "🌲 Birch" },
  { id: "paper-dark", label: "🪵 Oak" },
] as const;

const initTheme = () => {
  const savedTheme = getStorage("theme");
  document.documentElement.setAttribute("data-theme", savedTheme);
  return savedTheme;
};

const changeTheme = (themeId: (typeof themes)[number]["id"]) => {
  document.documentElement.setAttribute("data-theme", themeId);
  setStorage("theme", themeId);
};

// Initialize theme on page load
const currentTheme = initTheme();

// Create and attach theme dropdown
document.addEventListener("DOMContentLoaded", () => {
  const container = document.getElementById("theme-toggle");
  if (container) {
    // Create select element
    const select = document.createElement("select");
    select.id = "theme-select";

    // Add options
    themes.forEach((theme) => {
      const option = document.createElement("option");
      option.value = theme.id;
      option.textContent = theme.label;
      if (theme.id === currentTheme) {
        option.selected = true;
      }
      select.appendChild(option);
    });

    // Add change listener
    select.addEventListener("change", (e) => {
      const target = e.target as HTMLSelectElement;
      changeTheme(target.value as (typeof themes)[number]["id"]);
    });

    // Replace container content with select
    container.innerHTML = "";
    container.appendChild(select);
  }
});
