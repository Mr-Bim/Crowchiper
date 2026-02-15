import { escapeHtml } from "../../shared/dom.ts";

declare const API_PATH: string;
declare const LOGIN_PATH: string;
declare const APP_PATH: string;

interface UserSummary {
  uuid: string;
  username: string;
  role: "user" | "admin";
  activated: boolean;
  created_at: string;
}

async function fetchUsers(): Promise<UserSummary[]> {
  const response = await fetch(`${API_PATH}/admin/users`, {
    credentials: "include",
  });

  if (response.status === 401 || response.status === 403) {
    window.location.href = LOGIN_PATH;
    return [];
  }

  if (!response.ok) {
    throw new Error(`Failed to load users (${response.status})`);
  }

  return response.json();
}

function renderUsers(users: UserSummary[]): void {
  const tbody = document.getElementById("users-tbody");
  const table = document.getElementById("users-table");
  const loading = document.getElementById("users-loading");

  if (!tbody || !table || !loading) return;

  loading.hidden = true;
  table.hidden = false;

  tbody.innerHTML = "";
  for (const user of users) {
    const row = document.createElement("tr");
    row.innerHTML = `
      <td>${escapeHtml(user.username)}</td>
      <td>${escapeHtml(user.role)}</td>
      <td>${user.activated ? "Active" : "Pending"}</td>
      <td>${formatDate(user.created_at)}</td>
    `;
    tbody.appendChild(row);
  }
}

function showError(message: string): void {
  const errorEl = document.getElementById("users-error");
  const loading = document.getElementById("users-loading");

  if (loading) loading.hidden = true;
  if (errorEl) {
    errorEl.textContent = message;
    errorEl.hidden = false;
  }
}

function formatDate(dateStr: string): string {
  try {
    const date = new Date(dateStr + "Z");
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return dateStr;
  }
}

function setupLogout(): void {
  const logoutBtn = document.getElementById("logout-btn");
  if (!logoutBtn) return;

  logoutBtn.addEventListener("click", async () => {
    try {
      await fetch(`${API_PATH}/tokens/logout`, {
        method: "POST",
        credentials: "include",
      });
    } catch {
      // Redirect even on error
    }
    window.location.href = LOGIN_PATH;
  });
}

function setupAppLink(): void {
  const appLink = document.getElementById("app-link");
  if (appLink instanceof HTMLAnchorElement) {
    appLink.href = APP_PATH;
  }
}

async function init(): Promise<void> {
  setupLogout();
  setupAppLink();

  try {
    const users = await fetchUsers();
    renderUsers(users);
  } catch (err) {
    showError(err instanceof Error ? err.message : "Failed to load users");
  }
}

document.addEventListener("DOMContentLoaded", init);
