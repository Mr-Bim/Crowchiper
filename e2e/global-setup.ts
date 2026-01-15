import type { FullConfig } from "@playwright/test";
import { getDefaultServer } from "./server.ts";

export default async function globalSetup(_config: FullConfig) {
  // Start the default server (lazy-loaded, will be reused by tests)
  const { baseUrl } = await getDefaultServer();
  console.log(`Crowchiper server started at ${baseUrl}`);
}
