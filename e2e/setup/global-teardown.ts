import type { FullConfig } from "@playwright/test";
import { shutdownAllServers } from "../utils/server.ts";

export default function globalTeardown(_config: FullConfig) {
  shutdownAllServers();
  console.log("Crowchiper servers stopped");
}
