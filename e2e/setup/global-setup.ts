import type { FullConfig } from "@playwright/test";
import { startAllServers } from "../utils/server.ts";

export default async function globalSetup(_config: FullConfig) {
  // Start all server configurations needed by tests
  // This ensures servers are ready before parallel test files run
  await startAllServers();
  console.log("All Crowchiper servers started");
}
