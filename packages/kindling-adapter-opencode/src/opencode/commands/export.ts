/**
 * /memory export command
 *
 * MIGRATION NOTE (PORT-019): this command serialized a portable export bundle
 * via the in-process service's `createExportBundle` / `serializeBundle`. The
 * thin {@link import('@eddacraft/kindling').kindling} client / daemon exposes NO
 * export endpoint, so the command cannot be wired to the daemon. It is kept as a
 * clearly-marked stub (so the command surface and its formatter stay stable)
 * that always reports the operation as unsupported. Re-enable once the daemon
 * grows an export endpoint (export is owned by the Rust CLI / a future daemon
 * route per the Rust-canonical design).
 */

/**
 * Export options
 */
export interface ExportOptions {
  /** Output file path (default: auto-generated) */
  outputPath?: string;
  /** Export description */
  description?: string;
  /** Include redacted observations */
  includeRedacted?: boolean;
}

/**
 * Export result
 */
export interface ExportResult {
  /** Path to exported file */
  filePath: string;
  /** Number of entities exported */
  stats: {
    observations: number;
    capsules: number;
    summaries: number;
    pins: number;
  };
  /** Error if any */
  error?: string;
}

/**
 * Execute /memory export command.
 *
 * STUB: the daemon does not expose an export endpoint, so this always returns an
 * unsupported result. See the module note.
 *
 * @param _options - Export options (ignored)
 * @returns Export result (always unsupported)
 */
export function memoryExport(_options: ExportOptions = {}): ExportResult {
  return {
    filePath: '',
    stats: { observations: 0, capsules: 0, summaries: 0, pins: 0 },
    error: 'export is not supported by the kindling daemon (no export endpoint)',
  };
}

/**
 * Format export result as human-readable text
 *
 * @param result - Export result
 * @returns Formatted export result
 */
export function formatExportResult(result: ExportResult): string {
  if (result.error) {
    return `❌ Export failed: ${result.error}`;
  }

  const lines: string[] = [];

  lines.push('📦 Export complete');
  lines.push('');
  lines.push(`File: ${result.filePath}`);
  lines.push('');
  lines.push('Exported:');
  lines.push(`  ${result.stats.observations} observations`);
  lines.push(`  ${result.stats.capsules} capsules`);
  lines.push(`  ${result.stats.summaries} summaries`);
  lines.push(`  ${result.stats.pins} pins`);

  return lines.join('\n');
}
