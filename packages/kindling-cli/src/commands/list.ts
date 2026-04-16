/**
 * List command - List entities (capsules, pins, observations)
 */

import { initializeService, handleError, formatJson, formatTimestamp, truncate } from '../utils.js';

interface ListOptions {
  db?: string;
  session?: string;
  repo?: string;
  limit?: string;
  json?: boolean;
}

export async function listCommand(entity: string, options: ListOptions): Promise<void> {
  try {
    const { service, db } = initializeService(options.db);
    const limit = parseInt(options.limit || '20', 10);

    const scopeIds = {
      sessionId: options.session,
      repoId: options.repo,
    };

    let results: Record<string, unknown>[] = [];

    switch (entity.toLowerCase()) {
      case 'capsules': {
        const conditions: string[] = [];
        const params: (string | number)[] = [];

        if (options.session) {
          conditions.push("json_extract(scope_ids, '$.sessionId') = ?");
          params.push(options.session);
        }
        if (options.repo) {
          conditions.push("json_extract(scope_ids, '$.repoId') = ?");
          params.push(options.repo);
        }

        const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
        const query = `
          SELECT id, type, intent, status, opened_at, closed_at, scope_ids
          FROM capsules
          ${whereClause}
          ORDER BY opened_at DESC
          LIMIT ?
        `;
        params.push(limit);
        results = db.prepare(query).all(...params) as Record<string, unknown>[];
        break;
      }

      case 'pins': {
        results = service.listPins(scopeIds) as unknown as Record<string, unknown>[];
        break;
      }

      case 'observations': {
        const conditions: string[] = [];
        const params: (string | number)[] = [];

        if (options.session) {
          conditions.push("json_extract(scope_ids, '$.sessionId') = ?");
          params.push(options.session);
        }
        if (options.repo) {
          conditions.push("json_extract(scope_ids, '$.repoId') = ?");
          params.push(options.repo);
        }

        const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
        const query = `
          SELECT id, kind, content, ts, scope_ids, redacted
          FROM observations
          ${whereClause}
          ORDER BY ts DESC
          LIMIT ?
        `;
        params.push(limit);
        results = db.prepare(query).all(...params) as Record<string, unknown>[];
        break;
      }

      default:
        throw new Error(
          `Unknown entity type: ${entity}. Valid types: capsules, pins, observations`,
        );
    }

    if (options.json) {
      console.log(formatJson(results, true));
    } else {
      console.log(`\n${entity.charAt(0).toUpperCase() + entity.slice(1)} (${results.length}):`);
      console.log('='.repeat(50) + '\n');

      if (results.length === 0) {
        console.log('No results found.\n');
      } else {
        results.forEach((result, i) => {
          console.log(`${i + 1}. ${result.id}`);

          if (entity === 'capsules') {
            console.log(`   Type: ${result.type}`);
            console.log(`   Intent: ${result.intent}`);
            console.log(`   Status: ${result.status}`);
            console.log(`   Opened: ${formatTimestamp(result.opened_at as number)}`);
            if (result.closed_at) {
              console.log(`   Closed: ${formatTimestamp(result.closed_at as number)}`);
            }
          } else if (entity === 'pins') {
            console.log(`   Target: ${result.targetType} ${result.targetId}`);
            if (result.note) {
              console.log(`   Note: ${result.note}`);
            }
            console.log(`   Created: ${formatTimestamp(result.createdAt as number)}`);
            if (result.expiresAt) {
              console.log(`   Expires: ${formatTimestamp(result.expiresAt as number)}`);
            }
          } else if (entity === 'observations') {
            console.log(`   Kind: ${result.kind}`);
            console.log(`   Content: ${truncate(result.content as string, 100)}`);
            console.log(`   Time: ${formatTimestamp(result.ts as number)}`);
            console.log(`   Redacted: ${result.redacted ? 'yes' : 'no'}`);
          }

          console.log('');
        });
      }
    }

    db.close();
  } catch (error) {
    handleError(error, options.json);
  }
}
