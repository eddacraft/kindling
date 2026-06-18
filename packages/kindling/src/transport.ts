/**
 * HTTP/1-over-Unix-domain-socket transport with auto-spawn.
 *
 * Uses Node's built-in `http` over a socket path — no third-party HTTP
 * dependency, no native modules. One connection per request (simple and robust;
 * no pooling), mirroring the Rust client's transport. Before a request, if the
 * socket is missing or the connect is refused, we spawn the daemon ONCE and
 * poll-connect until it is reachable or the connect budget elapses.
 */

import { spawn } from 'node:child_process';
import { connect, type Socket } from 'node:net';
import { request as httpRequest } from 'node:http';
import { dirname } from 'node:path';

import type { ResolvedConfig } from './config.js';
import { DaemonUnavailableError } from './errors.js';

/** Header carrying the project root for per-project DB routing. */
export const PROJECT_HEADER = 'x-kindling-project';

/** A request to dispatch. */
export interface OutgoingRequest {
  method: string;
  path: string;
  /** Whether to send the `X-Kindling-Project` header (false for `/v1/health`). */
  project: boolean;
  /** Pre-serialized JSON body, or `undefined` for bodyless requests. */
  body?: string;
}

/** A decoded HTTP response: status plus the raw body string. */
export interface RawResponse {
  status: number;
  body: string;
}

/** Whether a Node socket error means "daemon not (yet) listening". */
function isAbsent(err: NodeJS.ErrnoException): boolean {
  return err.code === 'ENOENT' || err.code === 'ECONNREFUSED';
}

/** Attempt a single UDS connect; resolves the connected socket or rejects. */
function tryConnect(socketPath: string): Promise<Socket> {
  return new Promise((resolvePromise, reject) => {
    const sock = connect(socketPath);
    const onError = (err: Error) => {
      sock.destroy();
      reject(err);
    };
    sock.once('error', onError);
    sock.once('connect', () => {
      sock.removeListener('error', onError);
      resolvePromise(sock);
    });
  });
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * Spawn the daemon once, detached. The `kindling serve` verb binds the socket
 * and blocks (idle-shutting-down later); detaching + unref is the
 * daemonization. The kindling home is the socket's parent directory, matching
 * the CLI's `--kindling-home` defaulting (parent of `--socket`).
 */
function spawnDaemon(cfg: ResolvedConfig): void {
  const bin = cfg.binaryPath ?? 'kindling';
  const home = dirname(cfg.socketPath);
  const child = spawn(
    bin,
    ['serve', '--socket', cfg.socketPath, '--kindling-home', home],
    { detached: true, stdio: 'ignore' },
  );
  child.on('error', () => {
    /* swallowed: a failed spawn surfaces as a never-reachable socket below */
  });
  child.unref();
}

/**
 * Ensure the daemon is reachable, auto-spawning once if configured.
 *
 * 1. Try to connect. On success, return the socket.
 * 2. If the socket is missing/refused and `autoSpawn`, spawn ONCE, then
 *    poll-connect every `pollIntervalMs` until success or `connectTimeoutMs`.
 * 3. Otherwise throw {@link DaemonUnavailableError}.
 */
async function ensureConnected(cfg: ResolvedConfig): Promise<Socket> {
  try {
    return await tryConnect(cfg.socketPath);
  } catch (err) {
    if (!isAbsent(err as NodeJS.ErrnoException)) {
      throw new DaemonUnavailableError(
        `connecting to ${cfg.socketPath}: ${(err as Error).message}`,
      );
    }
  }

  if (!cfg.autoSpawn) {
    throw new DaemonUnavailableError(
      `socket ${cfg.socketPath} not reachable and autoSpawn is disabled`,
    );
  }

  try {
    spawnDaemon(cfg);
  } catch (err) {
    throw new DaemonUnavailableError(
      `failed to spawn kindling daemon: ${(err as Error).message}`,
    );
  }

  const deadline = Date.now() + cfg.connectTimeoutMs;
  let lastErr = 'unknown';
  for (;;) {
    try {
      return await tryConnect(cfg.socketPath);
    } catch (err) {
      const e = err as NodeJS.ErrnoException;
      if (!isAbsent(e)) {
        throw new DaemonUnavailableError(
          `connecting to ${cfg.socketPath} after spawn: ${e.message}`,
        );
      }
      lastErr = e.message;
    }
    if (Date.now() >= deadline) {
      throw new DaemonUnavailableError(
        `daemon socket ${cfg.socketPath} did not become reachable within ` +
          `${cfg.connectTimeoutMs}ms after spawn (${lastErr})`,
      );
    }
    await sleep(cfg.pollIntervalMs);
  }
}

/**
 * Connect to the daemon (spawning + polling as needed), send one request over
 * the established socket, and collect the response.
 */
export async function request(
  cfg: ResolvedConfig,
  req: OutgoingRequest,
): Promise<RawResponse> {
  const socket = await ensureConnected(cfg);

  const headers: Record<string, string> = {
    host: 'kindling.local',
    'content-type': 'application/json',
  };
  if (req.project) {
    headers[PROJECT_HEADER] = cfg.projectRoot;
  }
  const body = req.body ?? '';
  if (body.length > 0) {
    headers['content-length'] = String(Buffer.byteLength(body));
  }

  return await new Promise<RawResponse>((resolvePromise, reject) => {
    const clientReq = httpRequest(
      {
        createConnection: () => socket,
        method: req.method,
        path: req.path,
        headers,
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          resolvePromise({
            status: res.statusCode ?? 0,
            body: Buffer.concat(chunks).toString('utf8'),
          });
        });
        res.on('error', reject);
      },
    );
    clientReq.on('error', (err) =>
      reject(new DaemonUnavailableError(`http transport error: ${err.message}`)),
    );
    if (body.length > 0) {
      clientReq.write(body);
    }
    clientReq.end();
  });
}
