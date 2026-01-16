import {
  test as base,
  expect,
  BrowserContext,
  Page,
  CDPSession,
} from "@playwright/test";
import { getServer, Server } from "./server.ts";
import config from "../../config.json" with { type: "json" };

/** App assets path from config.json (e.g., "/fiery-sparrow") */
export const APP_PATH = config.assets;

/** Build the full app URL from a base URL */
export function appUrl(baseUrl: string): string {
  return `${baseUrl}${APP_PATH}`;
}

/**
 * Generate a unique test ID for use in usernames.
 * Uses a combination of timestamp and random string to ensure uniqueness
 * across parallel test runs.
 */
export function uniqueTestId(): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substring(2, 8);
  return `${timestamp}${random}`;
}

interface TestFixtures {
  context: BrowserContext;
  page: Page;
  cdpSession: CDPSession;
  /** Default server base URL */
  baseUrl: string;
  /** Get base URL for a specific server configuration */
  getServerUrl: (server: Server) => Promise<string>;
  /** Unique ID for this test, useful for creating unique usernames */
  testId: string;
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
    const { baseUrl } = await getServer(Server.Default);
    await use(baseUrl);
  },

  // Get base URL for a specific server configuration
  // eslint-disable-next-line no-empty-pattern
  getServerUrl: async ({}, use) => {
    await use(async (server: Server) => {
      const { baseUrl } = await getServer(server);
      return baseUrl;
    });
  },

  // Unique ID for this test
  // eslint-disable-next-line no-empty-pattern
  testId: async ({}, use) => {
    await use(uniqueTestId());
  },
});

/** Options for creating a test user */
export interface CreateUserOptions {
  /** The browser context to use */
  context: BrowserContext;
  /** Base URL for the server */
  baseUrl: string;
  /** Username for the user (should include testId for uniqueness) */
  username: string;
  /** Whether to enable encryption (default: true) */
  enableEncryption?: boolean;
}

/** Result of creating a test user */
export interface CreateUserResult {
  /** The page with the logged-in user */
  page: Page;
  /** The CDP session for the page */
  cdpSession: CDPSession;
  /** The PRF output used for encryption (if enabled) */
  prfOutput: string | null;
}

/**
 * Create a test user by registering and optionally setting up encryption.
 * Use this in beforeAll to create a user that can be reused across tests.
 *
 * @example
 * ```typescript
 * let userPage: Page;
 * let username: string;
 *
 * test.beforeAll(async ({ browser }) => {
 *   const context = await browser.newContext();
 *   username = `mytest_${uniqueTestId()}`;
 *   const result = await createUser({
 *     context,
 *     baseUrl: "http://localhost:7291",
 *     username,
 *     enableEncryption: true,
 *   });
 *   userPage = result.page;
 * });
 *
 * test.afterAll(async () => {
 *   await userPage?.context().close();
 * });
 * ```
 */
export async function createUser(
  options: CreateUserOptions,
): Promise<CreateUserResult> {
  const { context, baseUrl, username, enableEncryption = true } = options;

  const page = await context.newPage();
  const cdpSession = await page.context().newCDPSession(page);
  await addVirtualAuthenticator(cdpSession);

  let prfOutput: string | null = null;

  if (enableEncryption) {
    prfOutput = generateTestPrfOutput();
    await injectTestPrfOutput(page, prfOutput);
    await injectTestUsername(page, username);
  }

  // Register the user
  await page.goto(`${baseUrl}/login/register.html`);
  await page.locator("#register-button").waitFor({ state: "visible" });
  await page.fill("#username", username);
  await page.click("#register-button");

  // Wait for redirect to setup-encryption page
  await page.waitForURL(new RegExp(`${APP_PATH}/setup-encryption.html`), {
    timeout: 10000,
  });

  if (enableEncryption) {
    // Test PRF and enable encryption
    const testPrfBtn = page.locator("#test-prf-btn");
    await testPrfBtn.waitFor({ state: "visible" });
    await testPrfBtn.click();

    const enableEncryptionBtn = page.locator("#enable-encryption-btn");
    await enableEncryptionBtn.waitFor({ state: "visible", timeout: 10000 });
    await enableEncryptionBtn.click();

    // Wait for redirect to app (URL may or may not include index.html)
    await page.waitForURL(new RegExp(`${APP_PATH}(/index\\.html)?$`), {
      timeout: 10000,
    });

    // Unlock
    const unlockBtn = page.locator("#unlock-btn");
    await unlockBtn.waitFor({ state: "visible" });
    await unlockBtn.click();

    // Wait for unlock to complete
    const unlockOverlay = page.locator("#unlock-overlay");
    await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });
  } else {
    // Skip encryption - click continue
    const testPrfBtn = page.locator("#test-prf-btn");
    await testPrfBtn.waitFor({ state: "visible" });
    await testPrfBtn.click();

    const continueLink = page.locator("#continue-link");
    await continueLink.waitFor({ state: "visible", timeout: 10000 });
    await continueLink.click();

    // Wait for redirect to app (URL may or may not include index.html)
    await page.waitForURL(new RegExp(`${APP_PATH}(/index\\.html)?$`), {
      timeout: 10000,
    });
  }

  return { page, cdpSession, prfOutput };
}

export { expect, setupWebAuthn, Server };
