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

async function setupWebAuthn(page: Page): Promise<CDPSession> {
  const client = await page.context().newCDPSession(page);

  await client.send("WebAuthn.enable");

  await client.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

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
