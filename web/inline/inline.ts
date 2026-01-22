// Calculate BASE_PATH from current URL by removing /login or app assets path
(() => {
  const path = window.location.pathname;
  let base = "";

  const assets = "/__APP_ASSETS__";
  // If we're on a /login page, remove /login and everything after
  if (path.includes("/login")) {
    const loginIndex = path.indexOf("/login");
    base = path.substring(0, loginIndex);
  }
  // Otherwise, remove /__APP_ASSETS__ and everything after (will be replaced by build)
  else if (path.includes(assets)) {
    const appIndex = path.indexOf(assets);
    base = path.substring(0, appIndex);
  }

  const appPath = `${base}${assets}`;
  (window as unknown as Record<string, string>).API_PATH = `${base}/api`;
  (window as unknown as Record<string, string>).LOGIN_PATH = `${base}/login`;
  (window as unknown as Record<string, string>).APP_PATH = appPath;

  // Provide __assetsPath for Vite's renderBuiltUrl runtime resolution
  // This allows dynamic imports to resolve correctly with runtime base path
  (
    window as unknown as Record<string, (filename: string) => string>
  ).__assetsPath = (filename: string) => {
    return `${base}${assets}/${filename}`;
  };

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

// Theme switching logic
const themes = [
  { id: "warm-light", label: "🔥 Ember" },
  { id: "scandi-dark", label: "🪨 Slate" },
  { id: "paper-light", label: "🌲 Birch" },
  { id: "paper-dark", label: "🪵 Oak" },
];

const initTheme = () => {
  const savedTheme = localStorage.getItem("theme");
  const themeIds = themes.map((t) => t.id);
  if (savedTheme && themeIds.includes(savedTheme)) {
    document.documentElement.setAttribute("data-theme", savedTheme);
    return savedTheme;
  } else {
    document.documentElement.setAttribute("data-theme", "warm-light");
    return "warm-light";
  }
};

const changeTheme = (themeId: string) => {
  document.documentElement.setAttribute("data-theme", themeId);
  localStorage.setItem("theme", themeId);
};

// Initialize theme on page load
const currentTheme = initTheme();

// Create and attach theme dropdown and settings menu
document.addEventListener("DOMContentLoaded", () => {
  const container = document.getElementById("theme-toggle");
  if (container) {
    // Create select element
    const select = document.createElement("select");
    select.id = "theme-select";
    select.style.cssText = `
			background: var(--surface);
			border: 1px solid var(--border);
			border-radius: 4px;
			cursor: pointer;
			color: var(--text);
		`;

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
      changeTheme(target.value);
    });

    // Replace container content with select
    container.innerHTML = "";
    container.appendChild(select);
  }

  // Settings menu toggle
  const settingsBtn = document.getElementById("settings-btn");
  const settingsMenu = document.getElementById("settings-menu");

  if (settingsBtn && settingsMenu) {
    settingsBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      const isOpen = !settingsMenu.hidden;
      settingsMenu.hidden = isOpen;
      settingsBtn.setAttribute("aria-expanded", String(!isOpen));
    });

    // Close menu when clicking outside
    document.addEventListener("click", (e) => {
      if (
        !settingsMenu.hidden &&
        !settingsMenu.contains(e.target as Node) &&
        !settingsBtn.contains(e.target as Node)
      ) {
        settingsMenu.hidden = true;
        settingsBtn.setAttribute("aria-expanded", "false");
      }
    });

    // Close menu on Escape
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && !settingsMenu.hidden) {
        settingsMenu.hidden = true;
        settingsBtn.setAttribute("aria-expanded", "false");
      }
    });
  }

  // Logout button
  const logoutBtn = document.getElementById("logout-btn");
  if (logoutBtn) {
    logoutBtn.addEventListener("click", async () => {
      try {
        const apiPath = (window as unknown as Record<string, string>).API_PATH;
        const loginPath = (window as unknown as Record<string, string>)
          .LOGIN_PATH;
        await fetch(`${apiPath}/tokens/logout`, {
          method: "POST",
          credentials: "include",
        });
        window.location.href = loginPath;
        window.location.href = loginPath;
      } catch {}
    });
  }
});
