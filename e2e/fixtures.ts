import {
  test as base,
  expect,
  BrowserContext,
  Page,
  CDPSession,
} from "@playwright/test";
import { getServer, getDefaultServer, ServerOptions } from "./server.ts";
import config from "../config.json" with { type: "json" };

/** App assets path from config.json (e.g., "/fiery-sparrow") */
export const APP_PATH = config.assets;

/** Build the full app URL from a base URL */
export function appUrl(baseUrl: string): string {
  return `${baseUrl}${APP_PATH}`;
}

interface TestFixtures {
  context: BrowserContext;
  page: Page;
  cdpSession: CDPSession;
  baseUrl: string;
  serverWithOptions: (options: ServerOptions) => Promise<{ baseUrl: string }>;
}

/** Options for virtual authenticator setup */
export interface VirtualAuthenticatorOptions {
  hasPrf?: boolean;
}

/** Add a virtual authenticator to an existing CDP session */
export async function addVirtualAuthenticator(
  client: CDPSession,
  options: VirtualAuthenticatorOptions = {},
): Promise<void> {
  const { hasPrf = true } = options;

  await client.send("WebAuthn.enable");

  await client.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      hasPrf,
      automaticPresenceSimulation: true,
    },
  });
}

/**
 * Generate a random 32-byte PRF output as base64url.
 * Used for testing since Chrome's virtual authenticator doesn't return PRF output.
 */
export function generateTestPrfOutput(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  // Convert to base64url
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

/**
 * Inject test PRF output into a page before navigation.
 * This is needed because Chrome's virtual authenticator doesn't actually return PRF output.
 */
export async function injectTestPrfOutput(
  page: Page,
  prfOutput: string,
): Promise<void> {
  await page.addInitScript((prf) => {
    (window as unknown as { __TEST_PRF_OUTPUT__: string }).__TEST_PRF_OUTPUT__ =
      prf;
  }, prfOutput);
}

/**
 * Inject test username into a page before navigation.
 * This is needed because Chrome's virtual authenticator doesn't support discoverable credentials.
 */
export async function injectTestUsername(
  page: Page,
  username: string,
): Promise<void> {
  await page.addInitScript((u) => {
    (window as unknown as { __TEST_USERNAME__: string }).__TEST_USERNAME__ = u;
  }, username);
}

/** Set up WebAuthn for a page, creating a new CDP session */
async function setupWebAuthn(page: Page): Promise<CDPSession> {
  const client = await page.context().newCDPSession(page);
  await addVirtualAuthenticator(client);
  return client;
}

export const test = base.extend<TestFixtures>({
  context: async ({ browser }, use) => {
    const context = await browser.newContext();
    await use(context);
    await context.close();
  },

  page: async ({ context }, use) => {
    const page = await context.newPage();
    await setupWebAuthn(page);
    await use(page);
    await page.close();
  },

  cdpSession: async ({ page }, use) => {
    const client = await page.context().newCDPSession(page);
    await use(client);
  },

  // Default server base URL
  // eslint-disable-next-line no-empty-pattern
  baseUrl: async ({}, use) => {
    const { baseUrl } = await getDefaultServer();
    await use(baseUrl);
  },

  // Get a server with specific options (lazy-loaded)
  // eslint-disable-next-line no-empty-pattern
  serverWithOptions: async ({}, use) => {
    await use(async (options: ServerOptions) => {
      return getServer(options);
    });
  },
});

export { expect, setupWebAuthn };
