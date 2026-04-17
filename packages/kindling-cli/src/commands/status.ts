/**
 * Status command - Show database status and statistics
 */

import { initializeService, handleError, formatJson, getDefaultDbPath } from '../utils.js';

interface StatusOptions {
  db?: string;
  json?: boolean;
}

export async function statusCommand(options: StatusOptions): Promise<void> {
  try {
    const dbPath = options.db || getDefaultDbPath();
    const { db } = initializeService(dbPath);

    // Get database info
    const dbInfo = db.pragma('database_list') as Array<{ name: string; file: string }>;
    const mainDb = dbInfo.find((d) => d.name === 'main');

    // Get entity counts
    const observationCount = db.prepare('SELECT COUNT(*) as count FROM observations').get() as {
      count: number;
    };
    const capsuleCount = db.prepare('SELECT COUNT(*) as count FROM capsules').get() as {
      count: number;
    };
    const summaryCount = db.prepare('SELECT COUNT(*) as count FROM summaries').get() as {
      count: number;
    };
    const pinCount = db.prepare('SELECT COUNT(*) as count FROM pins').get() as { count: number };

    // Get redacted count
    const redactedCount = db
      .prepare('SELECT COUNT(*) as count FROM observations WHERE redacted = 1')
      .get() as { count: number };

    // Get open capsules count
    const openCapsulesCount = db
      .prepare("SELECT COUNT(*) as count FROM capsules WHERE status = 'open'")
      .get() as { count: number };

    // Get latest activity timestamp
    const latestActivity = db.prepare('SELECT MAX(ts) as ts FROM observations').get() as {
      ts: number | null;
    };

    // Get database size
    const pageCount = db.pragma('page_count') as number;
    const pageSize = db.pragma('page_size') as number;
    const dbSizeBytes = pageCount * pageSize;
    const dbSizeMB = (dbSizeBytes / (1024 * 1024)).toFixed(2);

    const status = {
      database: {
        path: mainDb?.file || dbPath,
        size: `${dbSizeMB} MB`,
        sizeBytes: dbSizeBytes,
      },
      counts: {
        observations: observationCount.count,
        capsules: capsuleCount.count,
        summaries: summaryCount.count,
        pins: pinCount.count,
        redacted: redactedCount.count,
        openCapsules: openCapsulesCount.count,
      },
      activity: {
        latestTimestamp: latestActivity.ts,
        latestDate: latestActivity.ts ? new Date(latestActivity.ts).toISOString() : null,
      },
    };

    if (options.json) {
      console.log(formatJson(status, true));
    } else {
      console.log('\nKindling Database Status');
      console.log('========================\n');
      console.log(`Database: ${status.database.path}`);
      console.log(`Size:     ${status.database.size}\n`);
      console.log('Entity Counts:');
      console.log(`  Observations: ${status.counts.observations}`);
      console.log(`  Capsules:     ${status.counts.capsules}`);
      console.log(`  Summaries:    ${status.counts.summaries}`);
      console.log(`  Pins:         ${status.counts.pins}`);
      console.log(`  Redacted:     ${status.counts.redacted}`);
      console.log(`  Open Capsules: ${status.counts.openCapsules}\n`);
      console.log('Latest Activity:');
      console.log(`  ${status.activity.latestDate || 'No activity yet'}\n`);
    }

    db.close();
  } catch (error) {
    handleError(error, options.json);
  }
}
