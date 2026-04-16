/**
 * CLI Utility Tests
 */

import { describe, it, expect } from 'vitest';
import { homedir } from 'os';
import { join } from 'path';
import { getDefaultDbPath, formatTimestamp, truncate, formatJson, formatError } from '../utils.js';

describe('getDefaultDbPath', () => {
  it('should return path in home directory', () => {
    const path = getDefaultDbPath();
    expect(path).toBe(join(homedir(), '.kindling', 'kindling.db'));
  });

  it('should end with kindling.db', () => {
    const path = getDefaultDbPath();
    expect(path).toMatch(/kindling\.db$/);
  });
});

describe('formatTimestamp', () => {
  it('should format timestamp as ISO date without milliseconds', () => {
    // Fixed timestamp: 2024-01-15T10:30:45.123Z
    const ts = new Date('2024-01-15T10:30:45.123Z').getTime();
    const formatted = formatTimestamp(ts);
    expect(formatted).toBe('2024-01-15 10:30:45');
  });

  it('should handle midnight timestamp', () => {
    const ts = new Date('2024-01-01T00:00:00.000Z').getTime();
    const formatted = formatTimestamp(ts);
    expect(formatted).toBe('2024-01-01 00:00:00');
  });

  it('should handle end of day timestamp', () => {
    const ts = new Date('2024-12-31T23:59:59.999Z').getTime();
    const formatted = formatTimestamp(ts);
    expect(formatted).toBe('2024-12-31 23:59:59');
  });
});

describe('truncate', () => {
  it('should not truncate short text', () => {
    expect(truncate('hello', 10)).toBe('hello');
    expect(truncate('hello', 5)).toBe('hello');
  });

  it('should truncate long text with ellipsis', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
    expect(truncate('hello world', 10)).toBe('hello w...');
  });

  it('should handle exact length', () => {
    expect(truncate('hello', 5)).toBe('hello');
    expect(truncate('hello', 6)).toBe('hello');
  });

  it('should handle empty string', () => {
    expect(truncate('', 10)).toBe('');
  });

  it('should handle very short maxLength', () => {
    expect(truncate('hello', 3)).toBe('...');
    expect(truncate('hello', 4)).toBe('h...');
  });
});

describe('formatJson', () => {
  it('should format object as compact JSON by default', () => {
    const obj = { foo: 'bar', baz: 123 };
    expect(formatJson(obj)).toBe('{"foo":"bar","baz":123}');
  });

  it('should format object as pretty JSON when specified', () => {
    const obj = { foo: 'bar' };
    const pretty = formatJson(obj, true);
    expect(pretty).toContain('\n');
    expect(pretty).toContain('  '); // Indentation
    expect(pretty).toBe('{\n  "foo": "bar"\n}');
  });

  it('should handle arrays', () => {
    const arr = [1, 2, 3];
    expect(formatJson(arr)).toBe('[1,2,3]');
  });

  it('should handle nested objects', () => {
    const obj = { a: { b: { c: 1 } } };
    expect(formatJson(obj)).toBe('{"a":{"b":{"c":1}}}');
  });

  it('should handle null and undefined', () => {
    expect(formatJson(null)).toBe('null');
    expect(formatJson(undefined)).toBe(undefined);
  });
});

describe('formatError', () => {
  it('should format Error object as plain text', () => {
    const error = new Error('Test error message');
    expect(formatError(error)).toBe('Error: Test error message');
  });

  it('should format Error object as JSON when specified', () => {
    const error = new Error('Test error message');
    const jsonOutput = formatError(error, true);
    expect(jsonOutput).toBe('{"error":"Test error message"}');
  });

  it('should format string error', () => {
    expect(formatError('Something went wrong')).toBe('Error: Something went wrong');
    expect(formatError('Something went wrong', true)).toBe('{"error":"Something went wrong"}');
  });

  it('should handle non-string, non-Error values', () => {
    expect(formatError(123)).toBe('Error: 123');
    expect(formatError({ custom: 'error' })).toBe('Error: [object Object]');
    expect(formatError(null)).toBe('Error: null');
  });
});
