/**
 * Minimal kindling adapter example.
 *
 * Demonstrates the four core operations every adapter needs:
 * openCapsule → appendObservation → retrieve → closeCapsule.
 *
 * Run from the repo root after building the workspace:
 *   pnpm install
 *   pnpm --filter @eddacraft/kindling run build
 *   pnpm --filter kindling-adapter-minimal start
 */

import { Kindling } from '@eddacraft/kindling';

const SESSION_ID = 'adapter-minimal-demo';
const REPO_ID = process.cwd();

async function main(): Promise<void> {
  const kindling = new Kindling({ projectRoot: REPO_ID });

  const health = await kindling.health();
  console.log(`Connected to kindling ${health.version} (schema v${health.schemaVersion})`);

  const capsule = await kindling.openCapsule({
    kind: 'session',
    intent: 'minimal adapter demonstration',
    scopeIds: { sessionId: SESSION_ID, repoId: REPO_ID },
  });
  console.log(`Opened capsule ${capsule.id}`);

  await kindling.appendObservation(
    {
      kind: 'message',
      content: 'JWT tokens expire after 15 minutes in this project',
      provenance: { source: 'adapter-minimal' },
      scopeIds: { sessionId: SESSION_ID, repoId: REPO_ID },
    },
    { capsuleId: capsule.id },
  );

  await kindling.appendObservation(
    {
      kind: 'error',
      content: 'JWT validation failed: token expired',
      provenance: { source: 'adapter-minimal', file: 'src/auth/validate.ts' },
      scopeIds: { sessionId: SESSION_ID, repoId: REPO_ID },
    },
    { capsuleId: capsule.id },
  );

  const result = await kindling.retrieve({
    query: 'JWT',
    scopeIds: { sessionId: SESSION_ID, repoId: REPO_ID },
  });

  console.log(`Retrieve returned ${result.candidates.length} candidate(s):`);
  for (const candidate of result.candidates) {
    const preview = candidate.entity.content.slice(0, 72).replace(/\s+/g, ' ');
    const label = 'kind' in candidate.entity ? candidate.entity.kind : 'summary';
    console.log(`  [${label} score=${candidate.score.toFixed(2)}] ${preview}`);
  }

  const closed = await kindling.closeCapsule(capsule.id, {
    summaryContent: 'Demonstrated minimal adapter lifecycle with JWT-related observations',
  });
  console.log(`Closed capsule ${closed.id} (status: ${closed.status})`);
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
