/**
 * kindling API Server
 *
 * HTTP API server for multi-agent concurrency. Holds a single database
 * connection and serializes all writes through a central coordination point.
 */

import Fastify, { FastifyInstance } from 'fastify';
import cors from '@fastify/cors';
import { KindlingService } from '@eddacraft/kindling-core';
import type {
  Observation,
  RetrieveOptions,
  OpenCapsuleOptions,
  CloseCapsuleOptions,
  ExportBundle,
} from '@eddacraft/kindling-core';

export interface ServerConfig {
  service: KindlingService;
  db: unknown; // better-sqlite3 Database type
  port?: number;
  host?: string;
  cors?: boolean;
}

export function createServer(config: ServerConfig): FastifyInstance {
  const { service, cors: enableCors = true } = config;

  const server = Fastify({
    logger: true,
  });

  // Enable CORS for browser clients
  if (enableCors) {
    server.register(cors, {
      origin: true,
    });
  }

  // Health check
  server.get('/health', async () => {
    return { status: 'ok', timestamp: Date.now() };
  });

  // Retrieve context
  server.post<{
    Body: RetrieveOptions;
  }>('/api/retrieve', async (request) => {
    const results = await service.retrieve(request.body);
    return results;
  });

  // Open capsule
  server.post<{
    Body: OpenCapsuleOptions;
  }>('/api/capsules', async (request) => {
    const capsule = service.openCapsule(request.body);
    return capsule;
  });

  // Close capsule
  server.post<{
    Params: { id: string };
    Body: CloseCapsuleOptions;
  }>('/api/capsules/:id/close', async (request) => {
    const capsule = service.closeCapsule(request.params.id, request.body);
    return capsule;
  });

  // Get capsule
  server.get<{
    Params: { id: string };
  }>('/api/capsules/:id', async (request, reply) => {
    const capsule = service.getCapsule(request.params.id);
    if (!capsule) {
      reply.code(404);
      return { error: 'Capsule not found' };
    }
    return capsule;
  });

  // Append observation
  server.post<{
    Body: {
      observation: Observation;
      capsuleId?: string;
    };
  }>('/api/observations', async (request, reply) => {
    const { observation, capsuleId } = request.body;
    service.appendObservation(observation, { capsuleId });
    reply.code(201);
    return { success: true };
  });

  // Create pin
  server.post<{
    Body: {
      targetType: 'observation' | 'summary';
      targetId: string;
      note?: string;
      scopeIds?: Record<string, string>;
      ttlMs?: number;
    };
  }>('/api/pins', async (request, reply) => {
    const pin = service.pin(request.body);
    reply.code(201);
    return pin;
  });

  // Remove pin
  server.delete<{
    Params: { id: string };
  }>('/api/pins/:id', async (request, reply) => {
    service.unpin(request.params.id);
    reply.code(204);
    return;
  });

  // Forget observation
  server.delete<{
    Params: { id: string };
  }>('/api/observations/:id', async (request, reply) => {
    service.forget(request.params.id);
    reply.code(204);
    return;
  });

  // Export
  server.post<{
    Body: {
      scopeIds?: Record<string, string>;
      includeRedacted?: boolean;
    };
  }>('/api/export', async (request) => {
    const bundle = service.export(request.body);
    return bundle;
  });

  // Import
  server.post<{
    Body: {
      bundle: ExportBundle;
    };
  }>('/api/import', async (request) => {
    const result = service.import(request.body.bundle);
    return result;
  });

  return server;
}

export async function startServer(config: ServerConfig): Promise<FastifyInstance> {
  const server = createServer(config);
  const { port = 8080, host = '127.0.0.1' } = config;

  await server.listen({ port, host });
  console.log(`🔥 kindling API server listening on http://${host}:${port}`);
  console.log(`📊 Health check: http://${host}:${port}/health`);

  return server;
}
