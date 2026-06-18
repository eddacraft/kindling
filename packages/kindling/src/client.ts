/**
 * The {@link Kindling} thin client — the TS mirror of the Rust
 * `kindling_client::Client`.
 *
 * A drop-in for the old in-process `KindlingService` surface (camelCase async
 * methods returning the generated domain types), but every call is one HTTP/1
 * request over the daemon's Unix domain socket. No native dependencies; the
 * daemon is auto-spawned on first use.
 */

import { resolveConfig, type KindlingOptions, type ResolvedConfig } from './config.js';
import { ApiError, SchemaMismatchError } from './errors.js';
import { request, type OutgoingRequest } from './transport.js';

import type { Capsule } from './generated/Capsule.js';
import type { CapsuleType } from './generated/CapsuleType.js';
import type { Observation } from './generated/Observation.js';
import type { ObservationInput } from './generated/ObservationInput.js';
import type { Pin } from './generated/Pin.js';
import type { PinTargetType } from './generated/PinTargetType.js';
import type { RetrieveOptions } from './generated/RetrieveOptions.js';
import type { RetrieveResult } from './generated/RetrieveResult.js';
import type { ScopeIds } from './generated/ScopeIds.js';

/** Result of `GET /v1/health`. */
export interface Health {
  /** Daemon package version. */
  version: string;
  /** Schema version the daemon's store reports. */
  schemaVersion: number;
  /** Project ids the daemon has touched this session. */
  projects: string[];
}

/** Arguments to {@link Kindling.openCapsule}. */
export interface OpenCapsuleArgs {
  kind: CapsuleType;
  intent: string;
  scopeIds?: ScopeIds;
  id?: string;
}

/** Arguments to {@link Kindling.closeCapsule}. */
export interface CloseCapsuleArgs {
  generateSummary?: boolean;
  summaryContent?: string;
  confidence?: number;
}

/** Arguments to {@link Kindling.appendObservation}. */
export interface AppendObservationArgs {
  capsuleId?: string;
  validate?: boolean;
}

/** Arguments to {@link Kindling.pin}. */
export interface PinArgs {
  targetType: PinTargetType;
  targetId: string;
  note?: string;
  ttlMs?: number;
  scopeIds?: ScopeIds;
}

/** Raw `/v1/health` JSON shape. */
interface HealthBody {
  version: string;
  schemaVersion: number;
  projects?: string[];
}

/** `/v1/context/*` response shape. */
interface ContextBody {
  additionalContext?: string | null;
}

/**
 * A thin async client for the kindling daemon.
 *
 * Construct once and reuse; the client holds no live socket (one connection per
 * call) and is safe to share.
 */
export class Kindling {
  readonly #config: ResolvedConfig;

  constructor(options: KindlingOptions = {}) {
    this.#config = resolveConfig(options);
  }

  /** The fully-resolved configuration this client was built with. */
  get config(): Readonly<ResolvedConfig> {
    return this.#config;
  }

  /**
   * `GET /v1/health` — version, schema version, and touched project ids.
   *
   * Verifies the daemon's `schemaVersion` matches the configured expected
   * version; throws {@link SchemaMismatchError} on mismatch (fail loud).
   */
  async health(): Promise<Health> {
    const body = await this.#call<HealthBody>('GET', '/v1/health', false, undefined, [200]);
    const expected = this.#config.expectedSchemaVersion;
    if (body.schemaVersion !== expected) {
      throw new SchemaMismatchError(expected, body.schemaVersion);
    }
    return {
      version: body.version,
      schemaVersion: body.schemaVersion,
      projects: body.projects ?? [],
    };
  }

  /** `POST /v1/capsules` — open a capsule. */
  async openCapsule(args: OpenCapsuleArgs): Promise<Capsule> {
    const body = {
      kind: args.kind,
      intent: args.intent,
      scopeIds: args.scopeIds ?? {},
      ...(args.id !== undefined ? { id: args.id } : {}),
    };
    return await this.#call<Capsule>('POST', '/v1/capsules', true, body, [201]);
  }

  /** `PATCH /v1/capsules/:id/close` — close a capsule. */
  async closeCapsule(id: string, args: CloseCapsuleArgs = {}): Promise<Capsule> {
    const body: Record<string, unknown> = {};
    if (args.generateSummary !== undefined) body.generateSummary = args.generateSummary;
    if (args.summaryContent !== undefined) body.summaryContent = args.summaryContent;
    if (args.confidence !== undefined) body.confidence = args.confidence;
    return await this.#call<Capsule>(
      'PATCH',
      `/v1/capsules/${encodeURIComponent(id)}/close`,
      true,
      body,
      [200],
    );
  }

  /**
   * `GET /v1/capsules/open?sessionId=…` — the open session capsule for
   * `sessionId`, or `null` when none is open.
   */
  async getOpenCapsule(sessionId: string): Promise<Capsule | null> {
    const path = `/v1/capsules/open?sessionId=${encodeURIComponent(sessionId)}`;
    return await this.#call<Capsule | null>('GET', path, true, undefined, [200]);
  }

  /**
   * `POST /v1/observations` — append an observation, optionally attaching it to
   * `capsuleId` and toggling service-side `validate` (default true).
   */
  async appendObservation(
    input: ObservationInput,
    args: AppendObservationArgs = {},
  ): Promise<Observation> {
    const body: Record<string, unknown> = { ...input };
    if (args.capsuleId !== undefined) body.capsuleId = args.capsuleId;
    if (args.validate !== undefined) body.validate = args.validate;
    return await this.#call<Observation>('POST', '/v1/observations', true, body, [201]);
  }

  /** `POST /v1/retrieve` — deterministic ranked retrieval. */
  async retrieve(options: RetrieveOptions): Promise<RetrieveResult> {
    return await this.#call<RetrieveResult>('POST', '/v1/retrieve', true, options, [200]);
  }

  /** `POST /v1/pins` — create a pin. */
  async pin(args: PinArgs): Promise<Pin> {
    const body: Record<string, unknown> = {
      targetType: args.targetType,
      targetId: args.targetId,
    };
    if (args.note !== undefined) body.note = args.note;
    if (args.ttlMs !== undefined) body.ttlMs = args.ttlMs;
    if (args.scopeIds !== undefined) body.scopeIds = args.scopeIds;
    return await this.#call<Pin>('POST', '/v1/pins', true, body, [201]);
  }

  /** `DELETE /v1/pins/:id` — remove a pin. */
  async unpin(id: string): Promise<void> {
    await this.#callNoContent('DELETE', `/v1/pins/${encodeURIComponent(id)}`, true, [204]);
  }

  /**
   * `POST /v1/observations/:id/forget` — redact an observation (content replaced
   * with `[redacted]`, `redacted` flag set). Resolves on `204`.
   *
   * A missing id throws {@link ApiError} with status `404` (the daemon maps the
   * store's `ObservationNotFound`). The `observationId` must be exact — prefix
   * resolution is a higher-layer concern.
   */
  async forget(observationId: string): Promise<void> {
    await this.#callNoContent(
      'POST',
      `/v1/observations/${encodeURIComponent(observationId)}/forget`,
      true,
      [204],
    );
  }

  /**
   * `POST /v1/context/session-start` — the assembled SessionStart injection
   * markdown, or `null` when there is nothing to inject. The project scope is
   * derived from this client's project root, mirroring the hook's
   * `{ repoId: <project root> }` filter.
   */
  async sessionStartContext(maxResults?: number): Promise<string | null> {
    const body: Record<string, unknown> = { scopeIds: this.#projectScope() };
    if (maxResults !== undefined) body.maxResults = maxResults;
    const resp = await this.#call<ContextBody>(
      'POST',
      '/v1/context/session-start',
      true,
      body,
      [200],
    );
    return resp.additionalContext ?? null;
  }

  /**
   * `POST /v1/context/pre-compact` — the assembled PreCompact injection
   * markdown, or `null` when there is nothing to inject.
   */
  async preCompactContext(): Promise<string | null> {
    const body = { scopeIds: this.#projectScope() };
    const resp = await this.#call<ContextBody>(
      'POST',
      '/v1/context/pre-compact',
      true,
      body,
      [200],
    );
    return resp.additionalContext ?? null;
  }

  /** A repo scope built from this client's project root. */
  #projectScope(): ScopeIds {
    return { repoId: this.#config.projectRoot };
  }

  // ---- internal request plumbing -----------------------------------------

  /** Send a request and decode a JSON body of one of the `expected` statuses. */
  async #call<T>(
    method: string,
    path: string,
    project: boolean,
    body: unknown,
    expected: number[],
  ): Promise<T> {
    const raw = await this.#send(method, path, project, body);
    this.#ensureStatus(raw, expected);
    try {
      return JSON.parse(raw.body) as T;
    } catch (err) {
      throw new ApiError(
        raw.status,
        `failed to decode daemon response: ${(err as Error).message}; body was ${raw.body}`,
      );
    }
  }

  /** Send a request that returns no body on success. */
  async #callNoContent(
    method: string,
    path: string,
    project: boolean,
    expected: number[],
  ): Promise<void> {
    const raw = await this.#send(method, path, project, undefined);
    this.#ensureStatus(raw, expected);
  }

  /** Serialize the body and dispatch through the transport. */
  async #send(
    method: string,
    path: string,
    project: boolean,
    body: unknown,
  ): Promise<{ status: number; body: string }> {
    const req: OutgoingRequest = {
      method,
      path,
      project,
      body: body === undefined ? undefined : JSON.stringify(body),
    };
    return await request(this.#config, req);
  }

  /** Throw {@link ApiError} if the status is not in `expected`. */
  #ensureStatus(raw: { status: number; body: string }, expected: number[]): void {
    if (expected.includes(raw.status)) return;
    let message: string;
    try {
      const parsed = JSON.parse(raw.body) as { error?: unknown };
      message =
        typeof parsed.error === 'string'
          ? parsed.error
          : raw.body.length > 0
            ? raw.body
            : `HTTP ${raw.status}`;
    } catch {
      message = raw.body.length > 0 ? raw.body : `HTTP ${raw.status}`;
    }
    throw new ApiError(raw.status, message);
  }
}
