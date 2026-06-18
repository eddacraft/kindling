/**
 * Serve command - Start API server for multi-agent access
 */

import { openDatabase } from '@eddacraft/kindling-store-sqlite';
import { SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { KindlingService } from '@eddacraft/kindling-core';
import { startServer } from '@eddacraft/kindling-server';
import { getDefaultDbPath } from '../utils.js';

export interface ServeOptions {
  db?: string;
  port?: string;
  host?: string;
  cors?: boolean;
}

export async function serveCommand(options: ServeOptions): Promise<void> {
  const dbPath = options.db || getDefaultDbPath();
  const port = options.port ? parseInt(options.port, 10) : 8080;
  const host = options.host || '127.0.0.1';

  console.log(`Starting kindling API server...`);
  console.log(`Database: ${dbPath}`);
  console.log(`Listening on: http://${host}:${port}`);
  console.log();

  const db = openDatabase({ path: dbPath });
  const store = new SqliteKindlingStore(db);
  const provider = new LocalFtsProvider(db);
  const service = new KindlingService({ store, provider });

  const server = await startServer({
    service,
    db,
    port,
    host,
    cors: options.cors !== false,
  });

  // Handle graceful shutdown
  const shutdown = async () => {
    console.log('\nShutting down gracefully...');
    await server.close();
    db.close();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}
