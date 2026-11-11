/**
 * VS Code extension entry point.
 *
 * Wires workspace save events and kindling commands into
 * {@link WorkspaceCapture}, which routes each document to the workspace
 * folder that contains it.
 */

import * as vscode from 'vscode';

import { Kindling } from '@eddacraft/kindling';

import { WorkspaceCapture, resolveWorkspaceRoot } from './workspace.js';

const OUTPUT_CHANNEL_NAME = 'Kindling';

let capture: WorkspaceCapture | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let sessionId: string | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
  context.subscriptions.push(outputChannel);

  sessionId = vscode.env.sessionId;
  capture = new WorkspaceCapture({
    sessionId,
    intent: 'VS Code editor session',
    createKindling: (projectRoot) => new Kindling(projectRoot ? { projectRoot } : {}),
  });

  const firstRoot = workspaceFolderPaths()[0];
  try {
    await capture.ensureSession(firstRoot);
    log(`Session started (${sessionId})`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    vscode.window.showWarningMessage(`Kindling: could not start session (${message})`);
  }

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(async (document) => {
      if (!capture || !sessionId) {
        return;
      }

      if (document.uri.scheme !== 'file') {
        return;
      }

      try {
        const result = await capture.captureSave(
          document.uri.fsPath,
          workspaceFolderPaths(),
          Date.now(),
        );

        if (result.error) {
          log(`File save skipped: ${result.error}`);
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log(`File save failed: ${message}`);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('kindling.search', async () => {
      if (!capture) {
        return;
      }

      const query = await vscode.window.showInputBox({
        prompt: 'Search kindling memory',
        placeHolder: 'Enter a search query',
      });

      if (!query) {
        return;
      }

      const repoId = repoIdForSearch();
      const kindling = capture.kindlingFor(repoId);

      try {
        const result = await kindling.retrieve({
          query,
          scopeIds: {
            ...(sessionId ? { sessionId } : {}),
            ...(repoId ? { repoId } : {}),
          },
          maxCandidates: 10,
        });

        const lines: string[] = [
          `Query: "${result.provenance.query}"`,
          `Found: ${result.provenance.totalCandidates} (showing ${result.provenance.returnedCandidates})`,
          '',
        ];

        for (const candidate of result.candidates) {
          const preview =
            candidate.entity.content.length > 200
              ? `${candidate.entity.content.slice(0, 200)}...`
              : candidate.entity.content;
          lines.push(`[${candidate.entity.id}] (${(candidate.score * 100).toFixed(0)}%)`);
          lines.push(preview);
          lines.push('');
        }

        if (result.candidates.length === 0) {
          lines.push('No results found.');
        }

        log(lines.join('\n'));
        outputChannel?.show(true);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        vscode.window.showErrorMessage(`Kindling search failed: ${message}`);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('kindling.logSelection', async () => {
      if (!capture || !sessionId) {
        return;
      }

      const editor = vscode.window.activeTextEditor;
      const selection = editor?.document.getText(editor.selection)?.trim();

      if (!editor || !selection) {
        vscode.window.showInformationMessage('Kindling: no text selected.');
        return;
      }

      if (editor.document.uri.scheme !== 'file') {
        vscode.window.showInformationMessage('Kindling: only file documents can be logged.');
        return;
      }

      const repoId = resolveWorkspaceRoot(editor.document.uri.fsPath, workspaceFolderPaths());
      if (!repoId) {
        vscode.window.showInformationMessage('Kindling: file is outside the workspace.');
        return;
      }

      try {
        const context = await capture.ensureSession(repoId);

        await capture.kindlingFor(repoId).appendObservation(
          {
            kind: 'message',
            content: selection,
            scopeIds: {
              sessionId,
              repoId,
            },
          },
          {
            capsuleId: context.activeCapsuleId,
          },
        );

        vscode.window.showInformationMessage('Kindling: selection logged.');
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        vscode.window.showErrorMessage(`Kindling log selection failed: ${message}`);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('kindling.status', async () => {
      if (!capture) {
        return;
      }

      try {
        const health = await capture.kindlingFor(workspaceFolderPaths()[0]).health();
        const active = capture.getActiveSessions();

        const lines = [
          'Kindling Status',
          '===============',
          '',
          `Daemon version: ${health.version}`,
          `Schema version: ${health.schemaVersion}`,
          `Projects touched: ${health.projects.length > 0 ? health.projects.join(', ') : '(none)'}`,
          `Active editor sessions: ${active.length > 0 ? active.join(', ') : '(none)'}`,
        ];

        log(lines.join('\n'));
        outputChannel?.show(true);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        vscode.window.showErrorMessage(`Kindling status failed: ${message}`);
      }
    }),
  );
}

export async function deactivate(): Promise<void> {
  if (capture) {
    try {
      await capture.closeAll({
        reason: 'extension_deactivate',
        summaryContent: 'VS Code editor session ended.',
        summaryConfidence: 0.5,
      });
      log('Session closed.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.warn(`Kindling: failed to close session on deactivate: ${message}`);
    }
  }

  capture = undefined;
  sessionId = undefined;
}

function workspaceFolderPaths(): string[] {
  return vscode.workspace.workspaceFolders?.map((folder) => folder.uri.fsPath) ?? [];
}

/**
 * Search scopes to the active document's workspace when it belongs to one,
 * otherwise the first folder (or unscoped when the window has no folder).
 */
function repoIdForSearch(): string | undefined {
  const folders = workspaceFolderPaths();
  const active = vscode.window.activeTextEditor?.document;
  if (active?.uri.scheme === 'file') {
    return resolveWorkspaceRoot(active.uri.fsPath, folders) ?? folders[0];
  }
  return folders[0];
}

function log(message: string): void {
  outputChannel?.appendLine(message);
}
