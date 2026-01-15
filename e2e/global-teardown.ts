import type { FullConfig } from "@playwright/test";
import { shutdownAllServers } from "./server.ts";

export default async function globalTeardown(_config: FullConfig) {
  await shutdownAllServers();
  console.log("Crowchiper servers stopped");
}
