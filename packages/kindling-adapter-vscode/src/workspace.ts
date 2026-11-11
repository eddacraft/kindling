/**
 * Per-document workspace routing for the editor extension.
 *
 * File saves must land in the kindling project that contains the file, not
 * the first workspace folder. Files outside every folder are skipped so they
 * cannot mix into another project's memory.
 */

import { isAbsolute, relative, resolve, sep } from 'node:path';

import type { Kindling } from '@eddacraft/kindling';

import {
  EditorSessionManager,
  type EditorEventResult,
  type EditorSessionContext,
  type EditorSessionEndSignals,
} from './session.js';

/**
 * Return the workspace folder that contains `filePath`, or `undefined` when
 * the file sits outside every folder. Nested folders pick the longest match
 * (the same rule as VS Code's `workspace.getWorkspaceFolder`). A sibling
 * path that merely shares a prefix is not a match.
 */
export function resolveWorkspaceRoot(
  filePath: string,
  workspaceFolders: readonly string[],
): string | undefined {
  const file = resolve(filePath);
  let best: string | undefined;
  let bestResolvedLength = -1;

  for (const folder of workspaceFolders) {
    const root = resolve(folder);
    if (!isPathInside(file, root)) {
      continue;
    }
    if (root.length > bestResolvedLength) {
      best = folder;
      bestResolvedLength = root.length;
    }
  }

  return best;
}

function isPathInside(filePath: string, folder: string): boolean {
  const rel = relative(folder, filePath);
  if (rel === '') {
    return true;
  }
  if (isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`)) {
    return false;
  }
  return true;
}

/** Builds a {@link Kindling} client for a workspace folder, or unscoped. */
export type KindlingFactory = (projectRoot?: string) => Kindling;

/** Options for {@link WorkspaceCapture}. */
export interface WorkspaceCaptureOptions {
  sessionId: string;
  intent?: string;
  createKindling: KindlingFactory;
}

interface ProjectHandle {
  kindling: Kindling;
  manager: EditorSessionManager;
}

/**
 * Routes editor capture to one Kindling client and session per workspace
 * folder. Each folder is a distinct kindling project (`X-Kindling-Project`),
 * so a multi-root window does not write every save into the first folder's
 * store.
 */
export class WorkspaceCapture {
  private readonly projects = new Map<string, ProjectHandle>();
  private readonly sessionId: string;
  private readonly intent: string;
  private readonly createKindling: KindlingFactory;

  constructor(options: WorkspaceCaptureOptions) {
    this.sessionId = options.sessionId;
    this.intent = options.intent ?? 'VS Code editor session';
    this.createKindling = options.createKindling;
  }

  /** Kindling client for `repoId`, creating one if needed. */
  kindlingFor(repoId?: string): Kindling {
    return this.handle(repoId).kindling;
  }

  /**
   * Open (or reuse) the editor session capsule for this workspace folder.
   */
  async ensureSession(repoId?: string): Promise<EditorSessionContext> {
    return this.handle(repoId).manager.onSessionStart({
      sessionId: this.sessionId,
      intent: this.intent,
      repoId,
    });
  }

  /**
   * Record a file save against the workspace folder that contains it.
   *
   * Returns `{ skipped: true, error: 'outside workspace' }` when the file
   * is not inside any of `workspaceFolders`.
   */
  async captureSave(
    filePath: string,
    workspaceFolders: readonly string[],
    timestamp?: number,
  ): Promise<EditorEventResult> {
    const repoId = resolveWorkspaceRoot(filePath, workspaceFolders);
    if (!repoId) {
      return { skipped: true, error: 'outside workspace' };
    }

    await this.ensureSession(repoId);
    return this.handle(repoId).manager.onFileSave({
      sessionId: this.sessionId,
      filePath,
      repoId,
      timestamp,
      ensureSession: true,
    });
  }

  /** Close every session this capture started. */
  async closeAll(signals?: EditorSessionEndSignals): Promise<void> {
    const handles = [...this.projects.values()];
    this.projects.clear();
    for (const handle of handles) {
      if (handle.manager.isSessionActive(this.sessionId)) {
        await handle.manager.onSessionEnd(this.sessionId, signals);
      }
    }
  }

  /** Session ids with an open capsule in any workspace. */
  getActiveSessions(): string[] {
    const ids = new Set<string>();
    for (const handle of this.projects.values()) {
      for (const id of handle.manager.getActiveSessions()) {
        ids.add(id);
      }
    }
    return [...ids];
  }

  private handle(repoId?: string): ProjectHandle {
    const key = repoId ?? '';
    let existing = this.projects.get(key);
    if (existing) {
      return existing;
    }

    const kindling = this.createKindling(repoId);
    existing = {
      kindling,
      manager: new EditorSessionManager(kindling),
    };
    this.projects.set(key, existing);
    return existing;
  }
}
