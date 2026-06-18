/**
 * Tests for kindling API Server
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import { KindlingService } from '@eddacraft/kindling-core';
import { SqliteKindlingStore } from '../../kindling-store-sqlite/src/store/sqlite.js';
import { LocalFtsProvider } from '../../kindling-provider-local/src/provider/local-fts.js';
import { openDatabase } from '../../kindling-store-sqlite/src/db/open.js';
import { createServer } from '../src/server.js';
import type { FastifyInstance } from 'fastify';

describe('Kindling API Server', () => {
  let db: Database.Database;
  let service: KindlingService;
  let server: FastifyInstance;

  beforeEach(async () => {
    db = openDatabase({ path: ':memory:' });
    const store = new SqliteKindlingStore(db);
    const provider = new LocalFtsProvider(db);
    service = new KindlingService({ store, provider });
    server = createServer({ service, db, cors: false });
    await server.ready();
  });

  afterEach(async () => {
    await server.close();
    db.close();
  });

  describe('GET /health', () => {
    it('should return health status', async () => {
      const response = await server.inject({
        method: 'GET',
        url: '/health',
      });

      expect(response.statusCode).toBe(200);
      const body = JSON.parse(response.body);
      expect(body.status).toBe('ok');
      expect(body.timestamp).toBeDefined();
    });
  });

  describe('GET /api/capsules/:id', () => {
    it('should return 404 for non-existent capsule', async () => {
      const response = await server.inject({
        method: 'GET',
        url: '/api/capsules/non-existent-id',
      });

      expect(response.statusCode).toBe(404);
      const body = JSON.parse(response.body);
      expect(body.error).toBe('Capsule not found');
    });

    it('should return capsule when it exists', async () => {
      // First create a capsule
      const createResponse = await server.inject({
        method: 'POST',
        url: '/api/capsules',
        payload: {
          type: 'session',
          intent: 'test',
          scopeIds: { sessionId: 'test-session' },
        },
      });

      expect(createResponse.statusCode).toBe(200);
      const created = JSON.parse(createResponse.body);

      // Now fetch it
      const response = await server.inject({
        method: 'GET',
        url: `/api/capsules/${created.id}`,
      });

      expect(response.statusCode).toBe(200);
      const body = JSON.parse(response.body);
      expect(body.id).toBe(created.id);
      expect(body.type).toBe('session');
      expect(body.intent).toBe('test');
      expect(body.status).toBe('open');
    });

    it('should return closed capsule', async () => {
      // Create and close a capsule
      const createResponse = await server.inject({
        method: 'POST',
        url: '/api/capsules',
        payload: {
          type: 'session',
          intent: 'test',
          scopeIds: { sessionId: 'test-session-2' },
        },
      });

      const created = JSON.parse(createResponse.body);

      await server.inject({
        method: 'POST',
        url: `/api/capsules/${created.id}/close`,
        payload: {},
      });

      // Fetch the closed capsule
      const response = await server.inject({
        method: 'GET',
        url: `/api/capsules/${created.id}`,
      });

      expect(response.statusCode).toBe(200);
      const body = JSON.parse(response.body);
      expect(body.id).toBe(created.id);
      expect(body.status).toBe('closed');
    });

    it('should include observationIds after appending observations', async () => {
      // Create a capsule
      const createResponse = await server.inject({
        method: 'POST',
        url: '/api/capsules',
        payload: {
          type: 'session',
          intent: 'test',
          scopeIds: { sessionId: 'test-session-3' },
        },
      });

      const created = JSON.parse(createResponse.body);
      expect(created.observationIds).toEqual([]);

      // Append an observation
      const obs1Response = await server.inject({
        method: 'POST',
        url: '/api/observations',
        payload: {
          observation: {
            id: 'obs-1',
            kind: 'message',
            content: 'First observation',
            provenance: {},
            ts: Date.now(),
            scopeIds: { sessionId: 'test-session-3' },
            redacted: false,
          },
          capsuleId: created.id,
        },
      });

      expect(obs1Response.statusCode).toBe(201);

      // Fetch the capsule - should have the observation
      const response1 = await server.inject({
        method: 'GET',
        url: `/api/capsules/${created.id}`,
      });

      expect(response1.statusCode).toBe(200);
      const body1 = JSON.parse(response1.body);
      expect(body1.observationIds).toEqual(['obs-1']);

      // Append another observation
      await server.inject({
        method: 'POST',
        url: '/api/observations',
        payload: {
          observation: {
            id: 'obs-2',
            kind: 'command',
            content: 'Second observation',
            provenance: { command: 'npm test' },
            ts: Date.now(),
            scopeIds: { sessionId: 'test-session-3' },
            redacted: false,
          },
          capsuleId: created.id,
        },
      });

      // Fetch the capsule again - should have both observations
      const response2 = await server.inject({
        method: 'GET',
        url: `/api/capsules/${created.id}`,
      });

      expect(response2.statusCode).toBe(200);
      const body2 = JSON.parse(response2.body);
      expect(body2.observationIds).toEqual(['obs-1', 'obs-2']);
    });
  });
});
