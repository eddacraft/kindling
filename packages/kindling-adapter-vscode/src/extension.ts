/**
 * VS Code extension entry point.
 *
 * Wires workspace save events and kindling commands into the
 * {@link EditorSessionManager}.
 */

import * as vscode from 'vscode';

import { Kindling } from '@eddacraft/kindling';

import { EditorSessionManager } from './session.js';

const OUTPUT_CHANNEL_NAME = 'Kindling';

let manager: EditorSessionManager | undefined;
let kindling: Kindling | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let sessionId: string | undefined;
let repoId: string | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
  context.subscriptions.push(outputChannel);

  repoId = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  sessionId = vscode.env.sessionId;

  kindling = new Kindling({
    ...(repoId ? { projectRoot: repoId } : {}),
  });
  manager = new EditorSessionManager(kindling);

  try {
    await manager.onSessionStart({
      sessionId,
      intent: 'VS Code editor session',
      repoId,
    });
    log(`Session started (${sessionId})`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    vscode.window.showWarningMessage(`Kindling: could not start session (${message})`);
  }

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(async (document) => {
      if (!manager || !sessionId) {
        return;
      }

      if (document.uri.scheme !== 'file') {
        return;
      }

      try {
        const result = await manager.onFileSave({
          sessionId,
          filePath: document.uri.fsPath,
          repoId,
          timestamp: Date.now(),
        });

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
      if (!kindling) {
        return;
      }

      const query = await vscode.window.showInputBox({
        prompt: 'Search kindling memory',
        placeHolder: 'Enter a search query',
      });

      if (!query) {
        return;
      }

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
      if (!kindling || !manager || !sessionId) {
        return;
      }

      const editor = vscode.window.activeTextEditor;
      const selection = editor?.document.getText(editor.selection)?.trim();

      if (!selection) {
        vscode.window.showInformationMessage('Kindling: no text selected.');
        return;
      }

      try {
        const context = manager.getSession(sessionId);
        if (!context) {
          await manager.onSessionStart({ sessionId, repoId });
        }

        await kindling.appendObservation(
          {
            kind: 'message',
            content: selection,
            scopeIds: {
              sessionId,
              ...(repoId ? { repoId } : {}),
            },
          },
          {
            capsuleId: manager.getSession(sessionId)?.activeCapsuleId,
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
      if (!kindling) {
        return;
      }

      try {
        const health = await kindling.health();
        const active = manager?.getActiveSessions() ?? [];

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
  if (manager && sessionId && manager.isSessionActive(sessionId)) {
    try {
      await manager.onSessionEnd(sessionId, {
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

  manager = undefined;
  kindling = undefined;
  sessionId = undefined;
  repoId = undefined;
}

function log(message: string): void {
  outputChannel?.appendLine(message);
}
