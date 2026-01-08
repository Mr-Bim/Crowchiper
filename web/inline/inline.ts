// Calculate BASE_PATH from current URL by removing /login or app assets path
(() => {
  const path = window.location.pathname;
  let base = "";

  // If we're on a /login page, remove /login and everything after
  if (path.includes("/login")) {
    const loginIndex = path.indexOf("/login");
    base = path.substring(0, loginIndex);
  }
  // Otherwise, remove /__APP_ASSETS__ and everything after (will be replaced by build)
  else if (path.includes("/__APP_ASSETS__")) {
    const appIndex = path.indexOf("/__APP_ASSETS__");
    base = path.substring(0, appIndex);
  }

  (window as unknown as Record<string, string>).API_PATH = `${base}/api`;
  (window as unknown as Record<string, string>).LOGIN_PATH = `${base}/login`;
  (window as unknown as Record<string, string>).APP_PATH =
    `${base}/__APP_ASSETS__`;

  // Provide __assetsPath for Vite's renderBuiltUrl runtime resolution
  // This allows dynamic imports to resolve correctly with runtime base path
  (
    window as unknown as Record<string, (filename: string) => string>
  ).__assetsPath = (filename: string) => {
    return `${base}/__APP_ASSETS__/${filename}`;
  };
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

// Create and attach theme dropdown
document.addEventListener("DOMContentLoaded", () => {
  const container = document.getElementById("theme-toggle");
  if (container) {
    // Create select element
    const select = document.createElement("select");
    select.id = "theme-select";
    select.style.cssText = `
			padding: 0.5rem 1rem;
			background: var(--surface);
			border: 1px solid var(--border);
			border-radius: 4px;
			cursor: pointer;
			font-size: 0.9rem;
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
});
