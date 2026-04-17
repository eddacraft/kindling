/**
 * Export/import commands - Manage data portability
 */

import { writeFileSync, readFileSync } from 'fs';
import { initializeService, handleError, formatJson } from '../utils.js';

interface ExportOptions {
  db?: string;
  session?: string;
  repo?: string;
  pretty?: boolean;
  json?: boolean;
}

interface ImportOptions {
  db?: string;
  dryRun?: boolean;
  json?: boolean;
}

export async function exportCommand(
  output: string | undefined,
  options: ExportOptions,
): Promise<void> {
  try {
    const { service, db } = initializeService(options.db);

    const exportOptions = {
      scope: {
        sessionId: options.session,
        repoId: options.repo,
      },
      metadata: {
        description: 'Kindling memory export',
        exportedAt: new Date().toISOString(),
      },
    };

    const bundle = service.export(exportOptions);
    const stats = service.getBundleStats(bundle);

    const outputPath = output || `kindling-export-${Date.now()}.json`;
    const jsonString = formatJson(bundle, options.pretty);

    writeFileSync(outputPath, jsonString, 'utf-8');

    if (options.json) {
      console.log(
        formatJson(
          {
            success: true,
            outputPath,
            stats,
          },
          true,
        ),
      );
    } else {
      console.log('\nExport successful');
      console.log(`Output: ${outputPath}`);
      console.log(`\nStatistics:`);
      console.log(`  Observations: ${stats.observations}`);
      console.log(`  Capsules:     ${stats.capsules}`);
      console.log(`  Summaries:    ${stats.summaries}`);
      console.log(`  Pins:         ${stats.pins}`);
      console.log(`  Size:         ${(stats.totalSize / 1024).toFixed(2)} KB\n`);
    }

    db.close();
  } catch (error) {
    handleError(error, options.json);
  }
}

export async function importCommand(file: string, options: ImportOptions): Promise<void> {
  try {
    const { service, db } = initializeService(options.db);

    const jsonString = readFileSync(file, 'utf-8');
    const result = service.importFromJson(jsonString, {
      dryRun: options.dryRun,
    });

    if (options.json) {
      console.log(formatJson(result, true));
    } else {
      console.log(
        `\n${options.dryRun ? 'Dry run' : 'Import'} ${result.errors.length > 0 ? 'completed with errors' : 'successful'}`,
      );
      console.log(`\nImported:`);
      console.log(`  Observations: ${result.observations}`);
      console.log(`  Capsules:     ${result.capsules}`);
      console.log(`  Summaries:    ${result.summaries}`);
      console.log(`  Pins:         ${result.pins}`);

      if (result.errors.length > 0) {
        console.log(`\nErrors (${result.errors.length}):`);
        result.errors.forEach((error) => console.log(`  - ${error}`));
      }

      console.log('');
    }

    db.close();
  } catch (error) {
    handleError(error, options.json);
  }
}
