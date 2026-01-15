import { spawn, ChildProcess } from "child_process";
import * as path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export interface ServerOptions {
  noSignup?: boolean;
  base?: string;
}

interface ServerInstance {
  proc: ChildProcess;
  port: number;
  baseUrl: string;
}

// Cache servers by their config key
const servers = new Map<string, ServerInstance>();

function optionsToKey(options: ServerOptions): string {
  return JSON.stringify({
    noSignup: options.noSignup ?? false,
    base: options.base ?? "",
  });
}

export async function getServer(
  options: ServerOptions = {},
): Promise<{ port: number; baseUrl: string }> {
  const key = optionsToKey(options);

  // Return existing server if available
  const existing = servers.get(key);
  if (existing) {
    return { port: existing.port, baseUrl: existing.baseUrl };
  }

  // Spawn new server
  const instance = await spawnServer(options);
  servers.set(key, instance);

  return { port: instance.port, baseUrl: instance.baseUrl };
}

async function spawnServer(options: ServerOptions): Promise<ServerInstance> {
  const binaryPath = path.resolve(__dirname, "../target/debug/crowchiper");

  const args = [
    "--port",
    "0",
    "--database",
    ":memory:",
    "--rp-id",
    "localhost",
  ];

  if (options.noSignup) {
    args.push("--no-signup");
  }

  if (options.base) {
    args.push("--base", options.base);
    args.push("--rp-origin", "http://localhost"); // Will be updated with actual port
  } else {
    args.push("--rp-origin", "http://localhost"); // Will be updated with actual port
  }

  const proc = spawn(binaryPath, args, {
    env: {
      ...process.env,
      JWT_SECRET: "test-jwt-secret-for-playwright-testing-minimum-32-chars",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });

  const port = await waitForReady(proc);

  const baseUrl = options.base
    ? `http://localhost:${port}${options.base}`
    : `http://localhost:${port}`;

  return { proc, port, baseUrl };
}

function waitForReady(proc: ChildProcess): Promise<number> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error("Server startup timeout"));
    }, 10000);

    let output = "";

    proc.stdout?.on("data", (data: Buffer) => {
      output += data.toString();
      const match = output.match(/CROWCHIPER_READY port=(\d+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(parseInt(match[1], 10));
      }
    });

    proc.stderr?.on("data", (data: Buffer) => {
      // Log stderr for debugging
      console.error("[server stderr]", data.toString());
    });

    proc.on("error", (err) => {
      clearTimeout(timeout);
      reject(err);
    });

    proc.on("exit", (code) => {
      if (code !== 0) {
        clearTimeout(timeout);
        reject(new Error(`Server exited with code ${code}`));
      }
    });
  });
}

export async function shutdownAllServers(): Promise<void> {
  for (const [key, instance] of servers) {
    instance.proc.kill();
    servers.delete(key);
  }
}

// Default server getter for convenience
export async function getDefaultServer(): Promise<{
  port: number;
  baseUrl: string;
}> {
  return getServer({});
}
