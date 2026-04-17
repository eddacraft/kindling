/**
 * Confidence Tracking
 *
 * Tracks execution history and calculates confidence scores for nodes.
 * Confidence reflects how reliable a node's output is for retrieval and reuse.
 */

/**
 * Result of a single node execution
 */
export interface ExecutionResult {
  /** Whether the execution succeeded */
  success: boolean;
  /** Timestamp of the execution */
  timestamp: number;
  /** Optional error message if failed */
  error?: string;
}

/**
 * Confidence state for a single node
 */
export interface ConfidenceState {
  /** Node identifier */
  nodeId: string;
  /** Current confidence score (0.0 - 1.0) */
  confidence: number;
  /** Total successful executions */
  successCount: number;
  /** Total failed executions */
  failureCount: number;
  /** Consecutive failures (resets on success) */
  consecutiveFailures: number;
  /** Recent execution history (newest first) */
  history: ExecutionResult[];
  /** Last updated timestamp */
  updatedAt: number;
}

/**
 * Configuration for confidence calculation
 */
export interface ConfidenceConfig {
  /** Maximum history entries to keep (default: 10) */
  historySize: number;
  /** Base confidence for new nodes (default: 0.5) */
  baseConfidence: number;
  /** Confidence increase per success (default: 0.1) */
  successIncrement: number;
  /** Confidence decrease per failure (default: 0.15) */
  failureDecrement: number;
  /** Minimum confidence floor (default: 0.1) */
  minConfidence: number;
  /** Maximum confidence ceiling (default: 0.95) */
  maxConfidence: number;
  /** Weight for recent executions vs older ones (default: 0.7) */
  recencyWeight: number;
}

const DEFAULT_CONFIG: ConfidenceConfig = {
  historySize: 10,
  baseConfidence: 0.5,
  successIncrement: 0.1,
  failureDecrement: 0.15,
  minConfidence: 0.1,
  maxConfidence: 0.95,
  recencyWeight: 0.7,
};

/**
 * Calculates confidence score from execution history.
 *
 * The algorithm considers:
 * - Overall success rate
 * - Recent execution trends (weighted more heavily)
 * - Consecutive failure penalty
 *
 * @param state - Current confidence state
 * @param config - Confidence calculation config
 * @returns Confidence score between 0.0 and 1.0
 */
export function calculateConfidence(
  state: Pick<ConfidenceState, 'successCount' | 'failureCount' | 'consecutiveFailures' | 'history'>,
  config: ConfidenceConfig = DEFAULT_CONFIG,
): number {
  const total = state.successCount + state.failureCount;

  // New node with no history - return base confidence
  if (total === 0) {
    return config.baseConfidence;
  }

  // Calculate overall success rate
  const overallRate = state.successCount / total;

  // Calculate recent success rate (weighted more heavily)
  const recentHistory = state.history.slice(0, Math.min(5, state.history.length));
  const recentSuccesses = recentHistory.filter((r) => r.success).length;
  const recentRate =
    recentHistory.length > 0 ? recentSuccesses / recentHistory.length : overallRate;

  // Blend overall and recent rates
  const blendedRate = overallRate * (1 - config.recencyWeight) + recentRate * config.recencyWeight;

  // Apply consecutive failure penalty
  const failurePenalty = Math.min(state.consecutiveFailures * 0.1, 0.3);

  // Calculate final confidence
  let confidence = blendedRate - failurePenalty;

  // Clamp to configured bounds
  confidence = Math.max(config.minConfidence, Math.min(config.maxConfidence, confidence));

  return Math.round(confidence * 100) / 100; // Round to 2 decimal places
}

/**
 * Tracks confidence across multiple nodes.
 *
 * @example
 * ```typescript
 * const tracker = new ConfidenceTracker();
 *
 * // Record successful execution
 * tracker.recordSuccess('run-tests');
 *
 * // Record failed execution
 * tracker.recordFailure('deploy-prod', 'Connection timeout');
 *
 * // Get confidence for a node
 * const confidence = tracker.getConfidence('run-tests'); // 0.6
 *
 * // Get full state for observation provenance
 * const state = tracker.getState('run-tests');
 * ```
 */
export class ConfidenceTracker {
  private states: Map<string, ConfidenceState> = new Map();
  private config: ConfidenceConfig;

  constructor(config: Partial<ConfidenceConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Records a successful execution for a node
   */
  recordSuccess(nodeId: string): ConfidenceState {
    const state = this.getOrCreateState(nodeId);

    state.successCount++;
    state.consecutiveFailures = 0;
    state.history.unshift({
      success: true,
      timestamp: Date.now(),
    });

    // Trim history to configured size
    if (state.history.length > this.config.historySize) {
      state.history = state.history.slice(0, this.config.historySize);
    }

    state.confidence = calculateConfidence(state, this.config);
    state.updatedAt = Date.now();

    return state;
  }

  /**
   * Records a failed execution for a node
   */
  recordFailure(nodeId: string, error?: string): ConfidenceState {
    const state = this.getOrCreateState(nodeId);

    state.failureCount++;
    state.consecutiveFailures++;
    state.history.unshift({
      success: false,
      timestamp: Date.now(),
      error,
    });

    // Trim history to configured size
    if (state.history.length > this.config.historySize) {
      state.history = state.history.slice(0, this.config.historySize);
    }

    state.confidence = calculateConfidence(state, this.config);
    state.updatedAt = Date.now();

    return state;
  }

  /**
   * Gets the current confidence score for a node
   */
  getConfidence(nodeId: string): number {
    const state = this.states.get(nodeId);
    return state?.confidence ?? this.config.baseConfidence;
  }

  /**
   * Gets the full confidence state for a node
   */
  getState(nodeId: string): ConfidenceState | undefined {
    return this.states.get(nodeId);
  }

  /**
   * Gets provenance metadata suitable for embedding in observations
   */
  getProvenanceMetadata(nodeId: string): Record<string, unknown> {
    const state = this.states.get(nodeId);
    if (!state) {
      return {
        confidence: this.config.baseConfidence,
        isNewNode: true,
      };
    }

    return {
      confidence: state.confidence,
      successCount: state.successCount,
      failureCount: state.failureCount,
      consecutiveFailures: state.consecutiveFailures,
      historyLength: state.history.length,
    };
  }

  /**
   * Resets tracking for a specific node
   */
  reset(nodeId: string): void {
    this.states.delete(nodeId);
  }

  /**
   * Clears all tracking data
   */
  clear(): void {
    this.states.clear();
  }

  private getOrCreateState(nodeId: string): ConfidenceState {
    let state = this.states.get(nodeId);
    if (!state) {
      state = {
        nodeId,
        confidence: this.config.baseConfidence,
        successCount: 0,
        failureCount: 0,
        consecutiveFailures: 0,
        history: [],
        updatedAt: Date.now(),
      };
      this.states.set(nodeId, state);
    }
    return state;
  }
}
