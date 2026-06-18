/**
 * kindling API Client
 *
 * TypeScript client for connecting to kindling API server.
 */

import type {
  Observation,
  Pin,
  Capsule,
  RetrieveOptions,
  RetrieveResult,
  OpenCapsuleOptions,
  CloseCapsuleSignals,
  ExportBundle,
  ImportResult,
} from '@eddacraft/kindling-core';

export interface KindlingApiClientConfig {
  baseUrl: string;
  timeout?: number;
}

export class KindlingApiClient {
  private baseUrl: string;
  private timeout: number;

  constructor(config: string | KindlingApiClientConfig) {
    if (typeof config === 'string') {
      this.baseUrl = config;
      this.timeout = 30000;
    } else {
      this.baseUrl = config.baseUrl;
      this.timeout = config.timeout ?? 30000;
    }
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: {
          'Content-Type': 'application/json',
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(`API error ${response.status}: ${error}`);
      }

      // Handle 204 No Content
      if (response.status === 204) {
        return undefined as T extends void ? T : never;
      }

      return (await response.json()) as T;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  async health(): Promise<{ status: string; timestamp: number }> {
    return this.request('GET', '/health');
  }

  async retrieve(options: RetrieveOptions): Promise<RetrieveResult> {
    return this.request('POST', '/api/retrieve', options);
  }

  async openCapsule(options: OpenCapsuleOptions): Promise<Capsule> {
    return this.request('POST', '/api/capsules', options);
  }

  async closeCapsule(capsuleId: string, signals?: CloseCapsuleSignals): Promise<Capsule> {
    return this.request('POST', `/api/capsules/${capsuleId}/close`, signals);
  }

  async appendObservation(
    observation: Observation,
    options?: { capsuleId?: string },
  ): Promise<void> {
    await this.request('POST', '/api/observations', {
      observation,
      capsuleId: options?.capsuleId,
    });
  }

  async pin(options: {
    targetType: 'observation' | 'summary';
    targetId: string;
    note?: string;
    scopeIds?: Record<string, string>;
    ttlMs?: number;
  }): Promise<Pin> {
    return this.request('POST', '/api/pins', options);
  }

  async unpin(pinId: string): Promise<void> {
    await this.request('DELETE', `/api/pins/${pinId}`);
  }

  async forget(observationId: string): Promise<void> {
    await this.request('DELETE', `/api/observations/${observationId}`);
  }

  async export(options?: {
    scopeIds?: Record<string, string>;
    includeRedacted?: boolean;
  }): Promise<ExportBundle> {
    return this.request('POST', '/api/export', options);
  }

  async import(
    bundle: ExportBundle,
    options?: { mode?: 'merge' | 'replace' },
  ): Promise<ImportResult> {
    return this.request('POST', '/api/import', {
      bundle,
      mode: options?.mode,
    });
  }
}
