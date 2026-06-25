import type { Kindling, ObservationKind, ObservationInput, ScopeIds } from '@eddacraft/kindling';

/**
 * Build an {@link ObservationInput} for the daemon.
 *
 * The thin client / daemon owns id, timestamp, and the `redacted` flag, so this
 * only carries the kind, content, provenance, and scope — unlike the old
 * in-process path which minted a full {@link Observation} locally.
 */
function buildObservation(
  kind: ObservationKind,
  content: string,
  provenance: Record<string, unknown>,
  scopeIds: ScopeIds,
): ObservationInput {
  return {
    kind,
    content,
    // Provenance values originate from arbitrary node params/outputs. They are
    // JSON-serialized over the wire; the generated `JsonValue`-indexed type is
    // narrower than this call site can statically guarantee, so cast at the
    // boundary (the daemon validates the payload).
    provenance: provenance as ObservationInput['provenance'],
    scopeIds,
  };
}

/**
 * Execution context threaded through a {@link KindlingNode}/{@link KindlingFlow}.
 *
 * Carries the daemon-backed {@link Kindling} client (replacing the old
 * in-process `PocketFlowStore`), the scope to tag observations with, and the
 * capsule id the active node is writing into.
 */
export interface KindlingNodeContext {
  kindling: Kindling;
  scopeIds: ScopeIds;
  capsuleId?: string;
}

export interface NodeMetadata {
  name: string;
  intent?: string;
}

type NonIterableObject = Partial<Record<string, unknown>> & { [Symbol.iterator]?: never };

function truncateOutput(output: unknown, maxLength: number = 2000): string {
  let str: string;
  if (output === undefined) {
    str = 'undefined';
  } else if (typeof output === 'string') {
    str = output;
  } else if (typeof output === 'bigint') {
    str = output.toString();
  } else {
    try {
      str = JSON.stringify(output) ?? 'undefined';
    } catch {
      str = '[unserializable]';
    }
  }
  if (str.length <= maxLength) return str;
  return str.substring(0, maxLength) + '... [truncated]';
}

export class BaseNode<S = unknown, P extends NonIterableObject = NonIterableObject> {
  protected _params: P = {} as P;
  protected _successors: Map<string, BaseNode> = new Map();

  async prep(_shared: S): Promise<unknown> {
    return undefined;
  }

  async exec(_prepRes: unknown): Promise<unknown> {
    return undefined;
  }

  async post(_shared: S, _prepRes: unknown, _execRes: unknown): Promise<string | undefined> {
    return undefined;
  }

  protected async _exec(prepRes: unknown): Promise<unknown> {
    return await this.exec(prepRes);
  }

  async _run(shared: S): Promise<string | undefined> {
    const p = await this.prep(shared);
    const e = await this._exec(p);
    return await this.post(shared, p, e);
  }

  async run(shared: S): Promise<string | undefined> {
    if (this._successors.size > 0) {
      console.warn("Node won't run successors. Use Flow.");
    }
    return await this._run(shared);
  }

  setParams(params: P): this {
    this._params = params;
    return this;
  }

  next<T extends BaseNode>(node: T): T {
    this.on('default', node);
    return node;
  }

  on(action: string, node: BaseNode): this {
    if (this._successors.has(action)) {
      console.warn(`Overwriting successor for action '${action}'`);
    }
    this._successors.set(action, node);
    return this;
  }

  getNextNode(action: string = 'default'): BaseNode | undefined {
    const nextAction = action || 'default';
    const next = this._successors.get(nextAction);
    if (!next && this._successors.size > 0) {
      console.warn(
        `Flow ends: '${nextAction}' not found in [${Array.from(this._successors.keys())}]`,
      );
    }
    return next;
  }

  clone(): this {
    const clonedNode = Object.create(Object.getPrototypeOf(this));
    Object.assign(clonedNode, this);
    clonedNode._params = { ...this._params };
    clonedNode._successors = new Map(this._successors);
    return clonedNode;
  }
}

export class Node<S = unknown, P extends NonIterableObject = NonIterableObject> extends BaseNode<
  S,
  P
> {
  maxRetries: number;
  wait: number;
  currentRetry: number = 0;

  constructor(maxRetries: number = 1, wait: number = 0) {
    super();
    this.maxRetries = maxRetries;
    this.wait = wait;
  }

  async execFallback(_prepRes: unknown, error: Error): Promise<unknown> {
    throw error;
  }

  protected override async _exec(prepRes: unknown): Promise<unknown> {
    for (this.currentRetry = 0; this.currentRetry < this.maxRetries; this.currentRetry++) {
      try {
        return await this.exec(prepRes);
      } catch (e) {
        if (this.currentRetry === this.maxRetries - 1) {
          return await this.execFallback(prepRes, e as Error);
        }
        if (this.wait > 0) {
          await new Promise((resolve) => setTimeout(resolve, this.wait * 1000));
        }
      }
    }
    return undefined;
  }
}

export class Flow<S = unknown, P extends NonIterableObject = NonIterableObject> extends BaseNode<
  S,
  P
> {
  start: BaseNode;

  constructor(start: BaseNode) {
    super();
    this.start = start;
  }

  protected async _orchestrate(shared: S, params?: P): Promise<void> {
    let current: BaseNode | undefined = this.start.clone();
    const p = params || this._params;
    while (current) {
      current.setParams(p);
      const action = await current._run(shared);
      current = current.getNextNode(action ?? 'default');
      current = current?.clone();
    }
  }

  override async _run(shared: S): Promise<string | undefined> {
    const pr = await this.prep(shared);
    await this._orchestrate(shared);
    return await this.post(shared, pr, undefined);
  }

  override async exec(_prepRes: unknown): Promise<unknown> {
    throw new Error("Flow can't exec.");
  }
}

export class KindlingNode<
  S extends KindlingNodeContext = KindlingNodeContext,
  P extends NonIterableObject = NonIterableObject,
> extends Node<S, P> {
  protected metadata: NodeMetadata;
  private nodeStartTime?: number;
  private capsuleId?: string;
  private sharedContext?: S;

  constructor(metadata: NodeMetadata, maxRetries: number = 1, wait: number = 0) {
    super(maxRetries, wait);
    this.metadata = metadata;
  }

  override async prep(shared: S): Promise<unknown> {
    this.nodeStartTime = Date.now();
    this.sharedContext = shared;

    const capsule = await shared.kindling.openCapsule({
      kind: 'pocketflow_node',
      intent: this.metadata.intent || 'general',
      scopeIds: shared.scopeIds,
    });

    this.capsuleId = capsule.id;
    shared.capsuleId = capsule.id;

    await shared.kindling.appendObservation(
      buildObservation(
        'node_start',
        `Node "${this.metadata.name}" started`,
        {
          nodeName: this.metadata.name,
          intent: this.metadata.intent,
          params: this._params,
        },
        shared.scopeIds,
      ),
      { capsuleId: this.capsuleId },
    );

    return undefined;
  }

  override async post(shared: S, _prepRes: unknown, execRes: unknown): Promise<string | undefined> {
    const duration = this.nodeStartTime ? Date.now() - this.nodeStartTime : 0;

    if (this.capsuleId) {
      await shared.kindling.appendObservation(
        buildObservation(
          'node_output',
          truncateOutput(execRes),
          {
            nodeName: this.metadata.name,
            outputType: typeof execRes,
            duration,
          },
          shared.scopeIds,
        ),
        { capsuleId: this.capsuleId },
      );

      await shared.kindling.appendObservation(
        buildObservation(
          'node_end',
          `Node "${this.metadata.name}" completed successfully`,
          {
            nodeName: this.metadata.name,
            duration,
            status: 'success',
          },
          shared.scopeIds,
        ),
        { capsuleId: this.capsuleId },
      );

      await shared.kindling.closeCapsule(this.capsuleId);
    }

    return undefined;
  }

  override async execFallback(_prepRes: unknown, error: Error): Promise<unknown> {
    if (this.capsuleId && this.sharedContext) {
      const duration = this.nodeStartTime ? Date.now() - this.nodeStartTime : 0;
      const shared = this.sharedContext;

      await shared.kindling.appendObservation(
        buildObservation(
          'node_error',
          `Node "${this.metadata.name}" failed: ${error.message}`,
          {
            nodeName: this.metadata.name,
            errorType: error.name,
            errorMessage: error.message,
            stack: error.stack,
            retryCount: this.currentRetry,
          },
          shared.scopeIds,
        ),
        { capsuleId: this.capsuleId },
      );

      await shared.kindling.appendObservation(
        buildObservation(
          'node_end',
          `Node "${this.metadata.name}" failed after ${this.currentRetry + 1} attempt(s)`,
          {
            nodeName: this.metadata.name,
            duration,
            status: 'error',
          },
          shared.scopeIds,
        ),
        { capsuleId: this.capsuleId },
      );

      await shared.kindling.closeCapsule(this.capsuleId);
    }

    throw error;
  }
}

export class KindlingFlow<
  S extends KindlingNodeContext = KindlingNodeContext,
  P extends NonIterableObject = NonIterableObject,
> extends Flow<S, P> {
  protected flowMetadata: NodeMetadata;
  private flowCapsuleId?: string;
  private flowStartTime?: number;

  constructor(start: KindlingNode<S, P>, metadata: NodeMetadata) {
    super(start);
    this.flowMetadata = metadata;
  }

  override async prep(shared: S): Promise<unknown> {
    this.flowStartTime = Date.now();

    const capsule = await shared.kindling.openCapsule({
      kind: 'pocketflow_node',
      intent: this.flowMetadata.intent || 'workflow',
      scopeIds: shared.scopeIds,
    });

    this.flowCapsuleId = capsule.id;

    await shared.kindling.appendObservation(
      buildObservation(
        'node_start',
        `Flow "${this.flowMetadata.name}" started`,
        {
          nodeName: this.flowMetadata.name,
          nodeType: 'flow',
          intent: this.flowMetadata.intent,
        },
        shared.scopeIds,
      ),
      { capsuleId: this.flowCapsuleId },
    );

    return undefined;
  }

  override async post(
    shared: S,
    _prepRes: unknown,
    _execRes: unknown,
  ): Promise<string | undefined> {
    const duration = this.flowStartTime ? Date.now() - this.flowStartTime : 0;

    if (this.flowCapsuleId) {
      await shared.kindling.appendObservation(
        buildObservation(
          'node_end',
          `Flow "${this.flowMetadata.name}" completed`,
          {
            nodeName: this.flowMetadata.name,
            nodeType: 'flow',
            duration,
            status: 'success',
          },
          shared.scopeIds,
        ),
        { capsuleId: this.flowCapsuleId },
      );

      await shared.kindling.closeCapsule(this.flowCapsuleId);
    }

    return undefined;
  }

  /**
   * Close the flow-level capsule on orchestration failure.
   *
   * Mirrors {@link KindlingNode.execFallback}: records a `node_error` followed
   * by an error `node_end`, then closes the capsule so a failed flow leaves no
   * open capsule behind. The error is rethrown so callers still observe the
   * failure.
   */
  private async closeFlowOnError(shared: S, error: Error): Promise<void> {
    const duration = this.flowStartTime ? Date.now() - this.flowStartTime : 0;

    if (this.flowCapsuleId) {
      await shared.kindling.appendObservation(
        buildObservation(
          'node_error',
          `Flow "${this.flowMetadata.name}" failed: ${error.message}`,
          {
            nodeName: this.flowMetadata.name,
            nodeType: 'flow',
            errorType: error.name,
            errorMessage: error.message,
            stack: error.stack,
          },
          shared.scopeIds,
        ),
        { capsuleId: this.flowCapsuleId },
      );

      await shared.kindling.appendObservation(
        buildObservation(
          'node_end',
          `Flow "${this.flowMetadata.name}" failed: ${error.message}`,
          {
            nodeName: this.flowMetadata.name,
            nodeType: 'flow',
            duration,
            status: 'error',
          },
          shared.scopeIds,
        ),
        { capsuleId: this.flowCapsuleId },
      );

      await shared.kindling.closeCapsule(this.flowCapsuleId);
    }
  }

  /**
   * Wrap {@link Flow._run} so the flow-level capsule is always closed.
   *
   * {@link Flow._run} opens the capsule via {@link prep}, then awaits
   * `_orchestrate` before calling {@link post}. If a child node fails,
   * `_orchestrate` rejects and `post` never runs — leaking the open capsule.
   * Catch that, write the failure end observation, close the capsule, and
   * rethrow so the failure still propagates.
   */
  override async _run(shared: S): Promise<string | undefined> {
    const pr = await this.prep(shared);
    try {
      await this._orchestrate(shared);
    } catch (error) {
      await this.closeFlowOnError(shared, error as Error);
      throw error;
    }
    return await this.post(shared, pr, undefined);
  }
}
