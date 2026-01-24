import { spawn, ChildProcess } from "child_process";
import * as path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

interface ServerOptions {
  noSignup?: boolean;
  base?: string;
}

interface ServerInstance {
  proc: ChildProcess;
  port: number;
  baseUrl: string;
}

/** Available server configurations */
export enum Server {
  Default = "default",
  NoSignup = "noSignup",
  BasePath = "basePath",
}

/** Server configuration definitions */
const SERVER_CONFIGS: Record<Server, ServerOptions> = {
  [Server.Default]: {},
  [Server.NoSignup]: { noSignup: true },
  [Server.BasePath]: { base: "/crow-chipher" },
};

// Cache servers by their config key
const servers = new Map<Server, ServerInstance>();

export async function getServer(
  server: Server,
): Promise<{ port: number; baseUrl: string }> {
  // Return existing server if available
  const existing = servers.get(server);
  if (existing) {
    return { port: existing.port, baseUrl: existing.baseUrl };
  }

  // Spawn new server
  const instance = await spawnServer(SERVER_CONFIGS[server]);
  servers.set(server, instance);

  return { port: instance.port, baseUrl: instance.baseUrl };
}

async function spawnServer(options: ServerOptions): Promise<ServerInstance> {
  const binaryPath = path.resolve(__dirname, "../../target/debug/crowchiper");

  const args = [
    "--port",
    "0",
    "--database",
    ":memory:",
    "--rp-id",
    "localhost",
    "--ip-header",
    "local",
  ];

  if (options.noSignup) {
    args.push("--no-signup");
  }

  if (options.base) {
    args.push("--base", options.base);
    args.push("--rp-origin", "http://localhost");
  } else {
    args.push("--rp-origin", "http://localhost");
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

export function shutdownAllServers(): void {
  for (const [, instance] of servers) {
    instance.proc.kill();
  }
  servers.clear();
}

// Handle process termination signals to clean up servers
process.on("SIGTERM", () => {
  shutdownAllServers();
  process.exit(0);
});

process.on("SIGINT", () => {
  shutdownAllServers();
  process.exit(0);
});

/**
 * Pre-start all server configurations.
 * Called from global setup to ensure servers are ready before parallel tests run.
 */
export async function startAllServers(): Promise<void> {
  await Promise.all(Object.values(Server).map((server) => getServer(server)));
}
