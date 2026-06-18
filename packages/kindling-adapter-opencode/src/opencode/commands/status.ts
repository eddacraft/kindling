/**
 * /memory status command
 *
 * Shows daemon status.
 *
 * MIGRATION NOTE (PORT-019): the old in-process command reported per-scope
 * counts (observations / capsules / summaries / pins) by querying the SQLite
 * store directly. The thin {@link import('@eddacraft/kindling').kindling} client
 * / daemon exposes no such aggregate-count endpoint — only `GET /v1/health`
 * (version, schema version, touched projects). This command is therefore
 * reduced to reporting daemon health; the count breakdown is dropped until the
 * daemon grows a stats endpoint. The previous `StatusStore` interface and count
 * fields are gone.
 */

/**
 * Health service interface.
 *
 * Satisfied by the {@link import('@eddacraft/kindling').kindling} thin client —
 * `memoryStatus` only needs its `health` method.
 */
export interface HealthService {
  health(): Promise<{
    version: string;
    schemaVersion: number;
    projects: string[];
  }>;
}

/**
 * Status result (daemon health).
 */
export interface StatusResult {
  /** Daemon package version. */
  version: string;
  /** Schema version reported by the daemon's store. */
  schemaVersion: number;
  /** Project ids the daemon has touched this session. */
  projects: string[];
}

/**
 * Execute /memory status command.
 *
 * @param service - Health service (the daemon client)
 * @returns Daemon status
 */
export async function memoryStatus(service: HealthService): Promise<StatusResult> {
  const health = await service.health();
  return {
    version: health.version,
    schemaVersion: health.schemaVersion,
    projects: health.projects,
  };
}

/**
 * Format status result as human-readable text.
 *
 * @param result - Status result
 * @returns Formatted status text
 */
export function formatStatus(result: StatusResult): string {
  const lines: string[] = [];

  lines.push('Memory Status');
  lines.push('=============');
  lines.push('');
  lines.push(`Daemon version: ${result.version}`);
  lines.push(`Schema version: ${result.schemaVersion}`);
  lines.push(
    `Projects touched: ${result.projects.length > 0 ? result.projects.join(', ') : '(none)'}`,
  );

  return lines.join('\n');
}
