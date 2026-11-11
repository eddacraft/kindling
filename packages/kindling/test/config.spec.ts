/**
 * Unit tests for config resolution in `config.ts`.
 *
 * These are pure (no daemon, no filesystem fixtures) and cover
 * `resolveConfig().binaryPath` precedence, including the fall-through to
 * `null` (which lets the transport spawn `kindling` from `PATH`), and
 * `resolveProjectRoot`'s `KINDLING_REPO_ROOT` containment check.
 */

import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { resolveConfig, resolveProjectRoot } from '../src/config.js';

describe('resolveConfig().binaryPath', () => {
  const original = process.env.KINDLING_BIN;
  afterEach(() => {
    if (original === undefined) delete process.env.KINDLING_BIN;
    else process.env.KINDLING_BIN = original;
  });

  it('prefers an explicit binaryPath option over everything', () => {
    process.env.KINDLING_BIN = '/env/kindling';
    expect(resolveConfig({ binaryPath: '/explicit/kindling' }).binaryPath).toBe(
      '/explicit/kindling',
    );
  });

  it('uses $KINDLING_BIN when no explicit option is given', () => {
    process.env.KINDLING_BIN = '/env/kindling';
    expect(resolveConfig().binaryPath).toBe('/env/kindling');
  });

  it('falls through to null when no binary can be resolved', () => {
    // In the test workspace no bundled `bin/kindling` exists, so resolution must reach the PATH fallback
    // sentinel (`null`) rather than returning a non-existent bundled path.
    delete process.env.KINDLING_BIN;
    expect(resolveConfig().binaryPath).toBeNull();
  });
});

describe('resolveProjectRoot KINDLING_REPO_ROOT', () => {
  const original = process.env.KINDLING_REPO_ROOT;
  afterEach(() => {
    if (original === undefined) delete process.env.KINDLING_REPO_ROOT;
    else process.env.KINDLING_REPO_ROOT = original;
  });

  it('honours the override for the exact root and a child path', () => {
    process.env.KINDLING_REPO_ROOT = '/work/repo';
    expect(resolveProjectRoot('/work/repo')).toBe('/work/repo');
    expect(resolveProjectRoot('/work/repo/src')).toBe('/work/repo');
  });

  it('rejects a sibling directory that shares a string prefix', () => {
    process.env.KINDLING_REPO_ROOT = '/work/repo';
    expect(resolveProjectRoot('/work/repo-copy')).not.toBe('/work/repo');
  });

  it('honours a relative override when cwd is under the resolved root', () => {
    process.env.KINDLING_REPO_ROOT = 'repo';
    expect(resolveProjectRoot('repo/src')).toBe('repo');
    process.env.KINDLING_REPO_ROOT = './repo';
    expect(resolveProjectRoot('repo/src')).toBe('./repo');
  });

  it('rejects a relative sibling that shares a string prefix', () => {
    process.env.KINDLING_REPO_ROOT = 'repo';
    expect(resolveProjectRoot('repo-copy')).not.toBe('repo');
  });
});

describe('resolveProjectRoot', () => {
  const temps: string[] = [];
  const saved: Record<string, string | undefined> = {};

  function remember(key: string): void {
    if (!(key in saved)) saved[key] = process.env[key];
  }

  function restoreEnv(): void {
    for (const [key, value] of Object.entries(saved)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    for (const key of Object.keys(saved)) delete saved[key];
  }

  afterEach(() => {
    restoreEnv();
    for (const dir of temps.splice(0)) {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  function gitEnv(): NodeJS.ProcessEnv {
    const env = { ...process.env };
    delete env.GIT_DIR;
    delete env.GIT_WORK_TREE;
    return env;
  }

  function git(cwd: string, args: string[]): void {
    execFileSync(
      'git',
      [
        '-c',
        'user.name=kindling',
        '-c',
        'user.email=k@example.com',
        '-c',
        'commit.gpgsign=false',
        '-c',
        'init.defaultBranch=main',
        ...args,
      ],
      { cwd, env: gitEnv(), stdio: 'pipe' },
    );
  }

  function isolatedToplevel(cwd: string): string {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], {
      cwd,
      encoding: 'utf8',
      env: gitEnv(),
    }).replace(/\r?\n$/, '');
  }

  function scratch(prefix: string): string {
    const dir = mkdtempSync(join(tmpdir(), prefix));
    temps.push(dir);
    return dir;
  }

  function initRepo(name = 'repo'): string {
    const repo = join(scratch('kindling-root-'), name);
    mkdirSync(repo, { recursive: true });
    git(repo, ['init', '-q']);
    return repo;
  }

  it('returns the git toplevel at the repo root and from a nested directory', () => {
    const repo = initRepo();
    const nested = join(repo, 'a', 'b');
    mkdirSync(nested, { recursive: true });
    const expected = isolatedToplevel(repo);
    expect(resolveProjectRoot(repo)).toBe(expected);
    expect(resolveProjectRoot(nested)).toBe(expected);
  });

  it('honours KINDLING_REPO_ROOT when cwd is under it', () => {
    const repo = initRepo();
    const nested = join(repo, 'src');
    mkdirSync(nested, { recursive: true });
    remember('KINDLING_REPO_ROOT');
    process.env.KINDLING_REPO_ROOT = repo;
    expect(resolveProjectRoot(nested)).toBe(repo);
  });

  it('ignores KINDLING_REPO_ROOT when cwd is outside it', () => {
    const repo = initRepo();
    const other = scratch('kindling-other-');
    remember('KINDLING_REPO_ROOT');
    process.env.KINDLING_REPO_ROOT = repo;
    expect(resolveProjectRoot(other)).toBe(resolve(other));
  });

  it('ignores GIT_WORK_TREE pointing at another checkout', () => {
    const repoA = initRepo('a');
    const repoB = initRepo('b');
    remember('GIT_WORK_TREE');
    process.env.GIT_WORK_TREE = repoB;
    expect(resolveProjectRoot(repoA)).toBe(isolatedToplevel(repoA));
  });

  it('ignores GIT_DIR pointing at another checkout from a nested directory', () => {
    const repoA = initRepo('a');
    const repoB = initRepo('b');
    const nested = join(repoA, 'sub');
    mkdirSync(nested, { recursive: true });
    remember('GIT_DIR');
    process.env.GIT_DIR = join(repoB, '.git');
    expect(resolveProjectRoot(nested)).toBe(isolatedToplevel(repoA));
  });

  it('linked worktree reports its own root even with Git overrides', () => {
    const dir = scratch('kindling-wt-');
    const repo = join(dir, 'repo');
    const wt = join(dir, 'wt');
    mkdirSync(repo);
    git(repo, ['init', '-q']);
    git(repo, ['commit', '-q', '--allow-empty', '-m', 'init']);
    git(repo, ['worktree', 'add', '-q', '-b', 'side', wt]);
    const nested = join(wt, 'sub');
    mkdirSync(nested);
    const other = initRepo('other');

    remember('GIT_DIR');
    remember('GIT_WORK_TREE');
    process.env.GIT_DIR = join(other, '.git');
    process.env.GIT_WORK_TREE = other;

    const expected = isolatedToplevel(wt);
    expect(resolveProjectRoot(wt)).toBe(expected);
    expect(resolveProjectRoot(nested)).toBe(expected);
    expect(expected).not.toBe(isolatedToplevel(repo));
  });

  it('preserves trailing whitespace in a repository path', () => {
    const dir = scratch('kindling-ws-');
    const repo = join(dir, 'repo ');
    mkdirSync(repo);
    if (repo.trimEnd() === repo) return;
    git(repo, ['init', '-q']);
    const root = resolveProjectRoot(repo);
    expect(root).toBe(isolatedToplevel(repo));
    expect(root).not.toBe(root.trim());
  });
});
