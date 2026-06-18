/**
 * OpenCode session lifecycle management
 *
 * Manages the lifecycle of session capsules through the daemon-backed
 * {@link Kindling} thin client (replacing the old in-process `CapsuleStore`):
 * - onSessionStart: opens a session capsule via the client
 * - onEvent: maps events and appends observations via the client
 * - onSessionEnd: closes the capsule (with an optional summary) via the client
 *
 * Every store interaction is now an async daemon round-trip, so the lifecycle
 * hooks are async.
 */

import type { Capsule, Observation, ObservationInput, Kindling } from '@eddacraft/kindling';
import type { OpenCodeEvent } from './events.js';
import { mapEvent } from './mapping.js';

/**
 * Session context tracking active session state
 */
export interface SessionContext {
  sessionId: string;
  repoId?: string;
  activeCapsuleId: string;
  eventCount: number;
}

/**
 * Options for starting a session
 */
export interface SessionStartOptions {
  sessionId: string;
  intent?: string;
  repoId?: string;
}

/**
 * Signals for ending a session
 */
export interface SessionEndSignals {
  reason?: string;
  summaryContent?: string;
  summaryConfidence?: number;
}

/**
 * Result of processing an event
 */
export interface EventProcessingResult {
  observation?: Observation;
  skipped?: boolean;
  error?: string;
}

/**
 * SessionManager manages OpenCode session lifecycles.
 *
 * Provides hooks for session start, event processing, and session end. Each
 * session gets its own capsule that collects observations. All persistence is
 * delegated to the daemon via the {@link Kindling} client.
 */
export class SessionManager {
  private activeSessions: Map<string, SessionContext> = new Map();

  constructor(private kindling: Kindling) {}

  /**
   * Start a new session.
   *
   * Opens a capsule for the session. If this manager already tracks the
   * session, returns the existing context. Otherwise it asks the daemon whether
   * an open capsule already exists for the session (crash recovery) before
   * opening a fresh one.
   */
  async onSessionStart(options: SessionStartOptions): Promise<SessionContext> {
    const { sessionId, intent = 'OpenCode session', repoId } = options;

    // Already tracked in-process.
    const existing = this.activeSessions.get(sessionId);
    if (existing) {
      return existing;
    }

    // Ask the daemon for an existing open capsule (recovers across restarts).
    const existingCapsule = await this.kindling.getOpenCapsule(sessionId);
    if (existingCapsule) {
      const context: SessionContext = {
        sessionId,
        repoId,
        activeCapsuleId: existingCapsule.id,
        eventCount: existingCapsule.observationIds.length,
      };
      this.activeSessions.set(sessionId, context);
      return context;
    }

    // Open a new capsule through the daemon.
    const capsule = await this.kindling.openCapsule({
      kind: 'session',
      intent,
      scopeIds: {
        sessionId,
        ...(repoId ? { repoId } : {}),
      },
    });

    const context: SessionContext = {
      sessionId,
      repoId,
      activeCapsuleId: capsule.id,
      eventCount: 0,
    };

    this.activeSessions.set(sessionId, context);
    return context;
  }

  /**
   * Process an event from the session.
   *
   * Maps the event to an observation input and appends it to the active capsule
   * via the daemon.
   */
  async onEvent(event: OpenCodeEvent): Promise<EventProcessingResult> {
    const context = this.activeSessions.get(event.sessionId);
    if (!context) {
      return {
        error: `No active session found for sessionId: ${event.sessionId}`,
      };
    }

    const mapResult = mapEvent(event);

    // Skip session lifecycle events (handled separately).
    if (mapResult.skip) {
      return { skipped: true };
    }

    if (mapResult.error) {
      return { error: mapResult.error };
    }

    if (!mapResult.observation) {
      return { error: 'Mapping produced no observation' };
    }

    // The daemon owns id and redaction; we pass the event timestamp so the
    // captured observation keeps the moment it actually happened.
    const input: ObservationInput = {
      ...mapResult.observation,
      ts: event.timestamp,
    };

    const observation = await this.kindling.appendObservation(input, {
      capsuleId: context.activeCapsuleId,
    });

    context.eventCount += 1;

    return { observation };
  }

  /**
   * End a session.
   *
   * Closes the active capsule for the session via the daemon, optionally asking
   * it to record a summary.
   */
  async onSessionEnd(sessionId: string, signals?: SessionEndSignals): Promise<Capsule> {
    const context = this.activeSessions.get(sessionId);
    if (!context) {
      throw new Error(`No active session found for sessionId: ${sessionId}`);
    }

    const closed = await this.kindling.closeCapsule(context.activeCapsuleId, {
      ...(signals?.summaryContent
        ? {
            // The daemon only persists a summary when generateSummary is set.
            generateSummary: true,
            summaryContent: signals.summaryContent,
            confidence: signals.summaryConfidence ?? 0.8,
          }
        : {}),
    });

    this.activeSessions.delete(sessionId);

    return closed;
  }

  /**
   * Get active session context.
   */
  getSession(sessionId: string): SessionContext | undefined {
    return this.activeSessions.get(sessionId);
  }

  /**
   * Check if session is active.
   */
  isSessionActive(sessionId: string): boolean {
    return this.activeSessions.has(sessionId);
  }

  /**
   * Get all active session IDs.
   */
  getActiveSessions(): string[] {
    return Array.from(this.activeSessions.keys());
  }
}
