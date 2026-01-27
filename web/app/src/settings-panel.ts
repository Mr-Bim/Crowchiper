/**
 * Settings panel for managing user sessions.
 * This module is lazy-loaded when the user opens the settings panel.
 * CSS is linked in index.html since the HTML structure is there.
 */

import { fetchWithAuth } from "./api/auth.ts";
import { getRequiredElement, getOptionalElement } from "../../shared/dom.ts";

declare const API_PATH: string;

interface TokenInfo {
  jti: string;
  last_ip: string | null;
  issued_at: string;
  expires_at: string;
  is_current: boolean;
}

interface ListTokensResponse {
  tokens: TokenInfo[];
}

function formatDate(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function renderSessions(tokens: TokenInfo[]): void {
  const container = getRequiredElement("sessions-list");

  if (tokens.length === 0) {
    container.innerHTML =
      '<div class="sessions-loading">No active sessions found.</div>';
    return;
  }

  // Sort: current session first, then by issued_at descending
  const sorted = [...tokens].sort((a, b) => {
    if (a.is_current && !b.is_current) return -1;
    if (!a.is_current && b.is_current) return 1;
    return new Date(b.issued_at).getTime() - new Date(a.issued_at).getTime();
  });

  container.innerHTML = sorted
    .map((token) => {
      const ipDisplay = token.last_ip || "Unknown IP";
      const currentBadge = token.is_current
        ? '<span class="session-current-badge" data-testid="test-session-current">Current</span>'
        : "";
      const revokeBtn = token.is_current
        ? ""
        : `<button class="session-revoke-btn" data-testid="test-session-revoke" data-jti="${token.jti}">Revoke</button>`;

      return `
      <div class="session-item" data-testid="test-session-item" data-current="${token.is_current}" data-jti="${token.jti}">
        <div class="session-info">
          <div class="session-ip">${ipDisplay}${currentBadge}</div>
          <div class="session-details">
            Created: ${formatDate(token.issued_at)} | Expires: ${formatDate(token.expires_at)}
          </div>
        </div>
        ${revokeBtn}
      </div>
    `;
    })
    .join("");

  // Add click handlers for revoke buttons
  container.querySelectorAll(".session-revoke-btn").forEach((btn) => {
    btn.addEventListener("click", handleRevoke);
  });
}

async function handleRevoke(event: Event): Promise<void> {
  const btn = event.target as HTMLButtonElement;
  const jti = btn.dataset.jti;
  if (!jti) return;

  btn.disabled = true;
  btn.textContent = "Revoking...";

  try {
    const response = await fetchWithAuth(`${API_PATH}/tokens/${jti}`, {
      method: "DELETE",
    });

    if (response.ok) {
      // Remove the session item from DOM
      const item = btn.closest(".session-item");
      item?.remove();

      // Check if list is now empty
      const container = getRequiredElement("sessions-list");
      if (!container.querySelector(".session-item")) {
        container.innerHTML =
          '<div class="sessions-loading">No other active sessions.</div>';
      }
    } else {
      btn.disabled = false;
      btn.textContent = "Revoke";
      console.error("Failed to revoke session");
    }
  } catch (err) {
    btn.disabled = false;
    btn.textContent = "Revoke";
    console.error("Failed to revoke session:", err);
  }
}

async function loadSessions(): Promise<void> {
  const container = getRequiredElement("sessions-list");
  container.innerHTML =
    '<div class="sessions-loading">Loading sessions...</div>';

  try {
    const response = await fetchWithAuth(`${API_PATH}/tokens`);

    if (!response.ok) {
      throw new Error("Failed to load sessions");
    }

    const data: ListTokensResponse = await response.json();
    renderSessions(data.tokens);
  } catch (err) {
    container.innerHTML =
      '<div class="sessions-error">Failed to load sessions. Please try again.</div>';
    console.error("Failed to load sessions:", err);
  }
}

export function openSettingsPanel(): void {
  const panel = getRequiredElement("settings-panel");
  panel.hidden = false;
  document.body.style.overflow = "hidden";
  loadSessions();
}

export function closeSettingsPanel(): void {
  const panel = getRequiredElement("settings-panel");
  panel.hidden = true;
  document.body.style.overflow = "";
}

let isSetup = false;

export function setupSettingsPanel(): void {
  // Only set up once
  if (isSetup) return;
  isSetup = true;

  const closeBtn = getOptionalElement("settings-panel-close");
  const panel = getOptionalElement("settings-panel");

  if (closeBtn) {
    closeBtn.addEventListener("click", closeSettingsPanel);
  }

  // Close on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && panel && !panel.hidden) {
      closeSettingsPanel();
    }
  });
}
