/**
 * Editor session lifecycle management
 *
 * Manages the lifecycle of session capsules through the daemon-backed
 * {@link Kindling} thin client:
 * - onSessionStart: opens a session capsule via the client
 * - onFileSave: appends a file_diff observation (optionally starting the session)
 * - onTerminalCommand: appends a command observation (optional stub)
 * - onSessionEnd: closes the capsule (with an optional summary) via the client
 */

import type { Capsule, Observation, ObservationInput, Kindling } from '@eddacraft/kindling';

/**
 * Session context tracking active editor session state.
 */
export interface EditorSessionContext {
  sessionId: string;
  repoId?: string;
  activeCapsuleId: string;
  observationCount: number;
}

/**
 * Options for starting an editor session.
 */
export interface EditorSessionStartOptions {
  sessionId: string;
  intent?: string;
  repoId?: string;
}

/**
 * File save event from the editor.
 */
export interface FileSaveEvent {
  sessionId: string;
  filePath: string;
  repoId?: string;
  timestamp?: number;
  diff?: string;
  additions?: number;
  deletions?: number;
  /**
   * When true, start a session if none is active. Used by the save handler so
   * capture can recover after activation failed to reach the daemon. One start
   * attempt per save; later saves retry if this attempt still fails.
   */
  ensureSession?: boolean;
}

/**
 * Terminal command event (optional capture hook).
 */
export interface TerminalCommandEvent {
  sessionId: string;
  command: string;
  exitCode?: number;
  stdout?: string;
  stderr?: string;
  repoId?: string;
  timestamp?: number;
}

/**
 * Signals for ending a session.
 */
export interface EditorSessionEndSignals {
  reason?: string;
  summaryContent?: string;
  summaryConfidence?: number;
}

/**
 * Result of processing an editor event.
 */
export interface EditorEventResult {
  observation?: Observation;
  skipped?: boolean;
  error?: string;
}

/**
 * EditorSessionManager manages VS Code editor session lifecycles.
 *
 * Provides hooks for session start, file saves, terminal commands, and session
 * end. Each session gets its own capsule that collects observations. All
 * persistence is delegated to the daemon via the {@link Kindling} client.
 */
export class EditorSessionManager {
  private activeSessions: Map<string, EditorSessionContext> = new Map();
  private pendingStarts: Map<string, Promise<EditorSessionContext>> = new Map();

  constructor(private kindling: Kindling) {}

  /**
   * Start a new editor session.
   *
   * Opens a capsule for the session. If this manager already tracks the
   * session, returns the existing context. Concurrent starts for the same
   * session share the in-flight work so only one daemon lookup or capsule
   * open runs. Otherwise it asks the daemon whether an open capsule already
   * exists for the session (crash recovery) before opening a fresh one.
   */
  async onSessionStart(options: EditorSessionStartOptions): Promise<EditorSessionContext> {
    const { sessionId } = options;

    const existing = this.activeSessions.get(sessionId);
    if (existing) {
      return existing;
    }

    const pending = this.pendingStarts.get(sessionId);
    if (pending) {
      return pending;
    }

    const start = this.startSession(options).finally(() => {
      this.pendingStarts.delete(sessionId);
    });
    this.pendingStarts.set(sessionId, start);
    return start;
  }

  private async startSession(options: EditorSessionStartOptions): Promise<EditorSessionContext> {
    const { sessionId, intent = 'Editor session', repoId } = options;

    const existingCapsule = await this.kindling.getOpenCapsule(sessionId);
    if (existingCapsule) {
      const context: EditorSessionContext = {
        sessionId,
        repoId,
        activeCapsuleId: existingCapsule.id,
        observationCount: existingCapsule.observationIds.length,
      };
      this.activeSessions.set(sessionId, context);
      return context;
    }

    const capsule = await this.kindling.openCapsule({
      kind: 'session',
      intent,
      scopeIds: {
        sessionId,
        ...(repoId ? { repoId } : {}),
      },
    });

    const context: EditorSessionContext = {
      sessionId,
      repoId,
      activeCapsuleId: capsule.id,
      observationCount: 0,
    };

    this.activeSessions.set(sessionId, context);
    return context;
  }

  /**
   * Record a file save as a file_diff observation.
   *
   * When {@link FileSaveEvent.ensureSession} is set and no session is tracked,
   * this starts one (the bounded recovery path after a failed activation start)
   * and then records the save. A failed start returns an error for this save
   * without throwing; the next save may try again.
   */
  async onFileSave(event: FileSaveEvent): Promise<EditorEventResult> {
    let context = this.activeSessions.get(event.sessionId);
    if (!context) {
      if (!event.ensureSession) {
        return {
          error: `No active session found for sessionId: ${event.sessionId}`,
        };
      }

      try {
        context = await this.onSessionStart({
          sessionId: event.sessionId,
          repoId: event.repoId,
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return {
          error: `No active session found for sessionId: ${event.sessionId}: ${message}`,
        };
      }
    }

    const content = formatFileSaveContent(event);
    const input: ObservationInput = {
      kind: 'file_diff',
      content,
      scopeIds: buildScopeIds(event.sessionId, event.repoId ?? context.repoId),
      provenance: {
        paths: [event.filePath],
        ...(event.additions !== undefined ? { additions: event.additions } : {}),
        ...(event.deletions !== undefined ? { deletions: event.deletions } : {}),
      } as ObservationInput['provenance'],
      ...(event.timestamp !== undefined ? { ts: event.timestamp } : {}),
    };

    const observation = await this.kindling.appendObservation(input, {
      capsuleId: context.activeCapsuleId,
    });

    context.observationCount += 1;
    return { observation };
  }

  /**
   * Record a terminal command as a command observation.
   *
   * Optional stub hook for terminal integrations; callers may wire this from a
   * terminal listener when available.
   */
  async onTerminalCommand(event: TerminalCommandEvent): Promise<EditorEventResult> {
    const context = this.activeSessions.get(event.sessionId);
    if (!context) {
      return {
        error: `No active session found for sessionId: ${event.sessionId}`,
      };
    }

    const content = formatCommandContent(event);
    const input: ObservationInput = {
      kind: 'command',
      content,
      scopeIds: buildScopeIds(event.sessionId, event.repoId ?? context.repoId),
      provenance: {
        cmd: extractCommandName(event.command),
        ...(event.exitCode !== undefined ? { exitCode: event.exitCode } : {}),
      } as ObservationInput['provenance'],
      ...(event.timestamp !== undefined ? { ts: event.timestamp } : {}),
    };

    const observation = await this.kindling.appendObservation(input, {
      capsuleId: context.activeCapsuleId,
    });

    context.observationCount += 1;
    return { observation };
  }

  /**
   * End a session.
   *
   * Closes the active capsule for the session via the daemon, optionally asking
   * it to record a summary.
   */
  async onSessionEnd(sessionId: string, signals?: EditorSessionEndSignals): Promise<Capsule> {
    const context = this.activeSessions.get(sessionId);
    if (!context) {
      throw new Error(`No active session found for sessionId: ${sessionId}`);
    }

    const closed = await this.kindling.closeCapsule(
      context.activeCapsuleId,
      signals?.summaryContent
        ? {
            generateSummary: true,
            summaryContent: signals.summaryContent,
            confidence: signals.summaryConfidence ?? 0.8,
          }
        : {},
    );

    this.activeSessions.delete(sessionId);
    return closed;
  }

  /** Get active session context. */
  getSession(sessionId: string): EditorSessionContext | undefined {
    return this.activeSessions.get(sessionId);
  }

  /** Check if session is active. */
  isSessionActive(sessionId: string): boolean {
    return this.activeSessions.has(sessionId);
  }

  /** Get all active session IDs. */
  getActiveSessions(): string[] {
    return Array.from(this.activeSessions.keys());
  }
}

function buildScopeIds(sessionId: string, repoId?: string) {
  return {
    sessionId,
    ...(repoId ? { repoId } : {}),
  };
}

function formatFileSaveContent(event: FileSaveEvent): string {
  const parts = [`Modified files:\n  ${event.filePath}`];

  if (event.additions !== undefined || event.deletions !== undefined) {
    parts.push(`+${event.additions ?? 0} -${event.deletions ?? 0}`);
  }

  if (event.diff) {
    parts.push(event.diff);
  }

  return parts.join('\n\n');
}

function formatCommandContent(event: TerminalCommandEvent): string {
  const parts = [`$ ${event.command}`];

  if (event.stdout) {
    parts.push(event.stdout);
  }

  if (event.stderr) {
    parts.push(`stderr: ${event.stderr}`);
  }

  if (event.exitCode !== undefined) {
    parts.push(`Exit code: ${event.exitCode}`);
  }

  return parts.join('\n\n');
}

function extractCommandName(command: string): string {
  const parts = command.trim().split(/\s+/);
  return parts[0] || command;
}
