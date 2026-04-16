/**
 * Tests for intent inference
 */

import { describe, it, expect } from 'vitest';
import { inferIntent, DEFAULT_INTENT_PATTERNS } from '../src/pocketflow/intent.js';

describe('inferIntent', () => {
  describe('testing intent', () => {
    it('should infer "test" from run-tests', () => {
      expect(inferIntent('run-tests')).toBe('test');
    });

    it('should infer "test" from runTests (camelCase)', () => {
      expect(inferIntent('runTests')).toBe('test');
    });

    it('should infer "test" from run_tests (snake_case)', () => {
      expect(inferIntent('run_tests')).toBe('test');
    });

    it('should infer "test" from validateInput', () => {
      expect(inferIntent('validateInput')).toBe('test');
    });

    it('should infer "test" from checkStatus', () => {
      expect(inferIntent('checkStatus')).toBe('test');
    });
  });

  describe('build intent', () => {
    it('should infer "build" from buildApp', () => {
      expect(inferIntent('buildApp')).toBe('build');
    });

    it('should infer "build" from compile-typescript', () => {
      expect(inferIntent('compile-typescript')).toBe('build');
    });

    it('should infer "build" from bundle_assets', () => {
      expect(inferIntent('bundle_assets')).toBe('build');
    });
  });

  describe('deploy intent', () => {
    it('should infer "deploy" from deployProduction', () => {
      expect(inferIntent('deployProduction')).toBe('deploy');
    });

    it('should infer "deploy" from publish-to-npm', () => {
      expect(inferIntent('publish-to-npm')).toBe('deploy');
    });

    it('should infer "deploy" from release_version', () => {
      expect(inferIntent('release_version')).toBe('deploy');
    });
  });

  describe('debug intent', () => {
    it('should infer "debug" from fixAuthBug', () => {
      expect(inferIntent('fixAuthBug')).toBe('debug');
    });

    it('should infer "debug" from debug-connection', () => {
      expect(inferIntent('debug-connection')).toBe('debug');
    });

    it('should infer "debug" from hotfix_payment', () => {
      expect(inferIntent('hotfix_payment')).toBe('debug');
    });
  });

  describe('feature intent', () => {
    it('should infer "feature" from implementFeature', () => {
      expect(inferIntent('implementFeature')).toBe('feature');
    });

    it('should infer "feature" from add-user-auth', () => {
      expect(inferIntent('add-user-auth')).toBe('feature');
    });

    it('should infer "feature" from create_new_endpoint', () => {
      expect(inferIntent('create_new_endpoint')).toBe('feature');
    });
  });

  describe('refactor intent', () => {
    it('should infer "refactor" from refactorDatabase', () => {
      expect(inferIntent('refactorDatabase')).toBe('refactor');
    });

    it('should infer "refactor" from cleanup-legacy', () => {
      expect(inferIntent('cleanup-legacy')).toBe('refactor');
    });
  });

  describe('process intent', () => {
    it('should infer "process" from processData', () => {
      expect(inferIntent('processData')).toBe('process');
    });

    it('should infer "process" from transform-json', () => {
      expect(inferIntent('transform-json')).toBe('process');
    });

    it('should infer "process" from parse_csv', () => {
      expect(inferIntent('parse_csv')).toBe('process');
    });
  });

  describe('analyze intent', () => {
    it('should infer "analyze" from analyzeMetrics', () => {
      expect(inferIntent('analyzeMetrics')).toBe('analyze');
    });

    it('should infer "analyze" from investigate-issue', () => {
      expect(inferIntent('investigate-issue')).toBe('analyze');
    });
  });

  describe('generate intent', () => {
    it('should infer "generate" from generateReport', () => {
      expect(inferIntent('generateReport')).toBe('generate');
    });

    it('should infer "generate" from scaffold-component', () => {
      expect(inferIntent('scaffold-component')).toBe('generate');
    });

    it('should infer "generate" from init_project', () => {
      expect(inferIntent('init_project')).toBe('generate');
    });
  });

  describe('unknown patterns', () => {
    it('should return "general" for unrecognized patterns', () => {
      expect(inferIntent('unknownNode')).toBe('general');
    });

    it('should return "general" for empty string', () => {
      expect(inferIntent('')).toBe('general');
    });

    it('should return "general" for random text', () => {
      expect(inferIntent('fooBarBaz')).toBe('general');
    });
  });

  describe('custom patterns', () => {
    it('should use custom patterns when provided', () => {
      const customPatterns = [{ keywords: ['custom', 'special'], intent: 'custom-intent' }];
      expect(inferIntent('customOperation', customPatterns)).toBe('custom-intent');
    });

    it('should return "general" if custom patterns do not match', () => {
      const customPatterns = [{ keywords: ['xyz'], intent: 'xyz-intent' }];
      expect(inferIntent('normalOperation', customPatterns)).toBe('general');
    });
  });

  describe('edge cases', () => {
    it('should handle PascalCase', () => {
      expect(inferIntent('RunTests')).toBe('test');
    });

    it('should handle UPPERCASE', () => {
      expect(inferIntent('RUNTESTS')).toBe('general'); // All caps doesn't split
    });

    it('should handle mixed separators', () => {
      expect(inferIntent('run_tests-now')).toBe('test');
    });

    it('should handle numbers', () => {
      expect(inferIntent('test123')).toBe('test');
    });
  });

  describe('DEFAULT_INTENT_PATTERNS', () => {
    it('should have patterns for all major intents', () => {
      const intents = DEFAULT_INTENT_PATTERNS.map((p) => p.intent);
      expect(intents).toContain('test');
      expect(intents).toContain('build');
      expect(intents).toContain('deploy');
      expect(intents).toContain('debug');
      expect(intents).toContain('feature');
      expect(intents).toContain('refactor');
      expect(intents).toContain('process');
      expect(intents).toContain('analyze');
      expect(intents).toContain('generate');
    });
  });
});
