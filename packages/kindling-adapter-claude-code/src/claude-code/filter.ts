/**
 * Content filtering and safety utilities for Claude Code adapter
 *
 * Provides filters to prevent accidental secret capture and reduce noise:
 * - Content truncation for large outputs
 * - Pattern-based secret detection
 * - File path filtering
 */

/**
 * Maximum content length before truncation (characters)
 */
export const MAX_CONTENT_LENGTH = 50000; // 50KB

/**
 * Maximum result length for tool results
 */
export const MAX_RESULT_LENGTH = 10000; // 10KB

/**
 * Common secret patterns to detect in content
 */
const SECRET_PATTERNS = [
  // API keys and tokens
  /['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?([^\s'"]+)['"]?/gi,
  // AWS keys
  /(?:AWS|aws)[-_]?(?:SECRET|secret)[-_]?(?:ACCESS|access)[-_]?(?:KEY|key)\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})['"]?/g,
  // Generic API tokens (long alphanumeric strings)
  /\b(?=.*[0-9])(?=.*[A-Za-z])[A-Za-z0-9]{32,}\b/g,
  // Bearer tokens
  /Bearer\s+([A-Za-z0-9\-._~+/]+=*)/gi,
  // Basic auth
  /Basic\s+([A-Za-z0-9+/]+=*)/gi,
  // Anthropic API keys
  /sk-ant-[A-Za-z0-9\-_]{90,}/g,
  // OpenAI API keys
  /sk-[A-Za-z0-9]{48,}/g,
];

/**
 * File paths that should be excluded from capture
 */
const EXCLUDED_PATHS = [
  /node_modules/,
  /\.git\//,
  /\.env$/,
  /\.pem$/,
  /\.key$/,
  /credentials/i,
  /secrets/i,
];

/**
 * Tools whose full results should not be captured
 */
const SKIP_RESULT_TOOLS = [
  'WebSearch', // Full search results are noisy
];

/**
 * Options for content filtering
 */
export interface FilterOptions {
  /** Maximum content length before truncation */
  maxLength?: number;
  /** Whether to detect and mask secrets */
  maskSecrets?: boolean;
  /** Whether to append truncation notice */
  showTruncationNotice?: boolean;
}

/**
 * Truncate content if it exceeds max length
 */
export function truncateContent(content: string, options: FilterOptions = {}): string {
  const { maxLength = MAX_CONTENT_LENGTH, showTruncationNotice = true } = options;

  if (content.length <= maxLength) {
    return content;
  }

  const truncated = content.substring(0, maxLength);

  if (showTruncationNotice) {
    const remaining = content.length - maxLength;
    return `${truncated}\n\n[Truncated ${remaining} characters]`;
  }

  return truncated;
}

/**
 * Detect if content contains potential secrets
 */
export function containsSecrets(content: string): boolean {
  return SECRET_PATTERNS.some((pattern) => {
    pattern.lastIndex = 0;
    return pattern.test(content);
  });
}

/**
 * Mask potential secrets in content
 */
export function maskSecrets(content: string): string {
  let masked = content;

  for (const pattern of SECRET_PATTERNS) {
    pattern.lastIndex = 0;
    masked = masked.replace(pattern, (match) => {
      if (match.includes(':') || match.includes('=')) {
        const parts = match.split(/[:=]/);
        return `${parts[0]}=[REDACTED]`;
      }
      return '[REDACTED]';
    });
  }

  return masked;
}

/**
 * Filter content with all safety rules
 */
export function filterContent(content: string, options: FilterOptions = {}): string {
  const { maskSecrets: shouldMask = true } = options;

  let filtered = content;

  if (shouldMask && containsSecrets(filtered)) {
    filtered = maskSecrets(filtered);
  }

  filtered = truncateContent(filtered, options);

  return filtered;
}

/**
 * Check if a file path should be excluded from capture
 */
export function isExcludedPath(path: string): boolean {
  return EXCLUDED_PATHS.some((pattern) => pattern.test(path));
}

/**
 * Check if a tool's result should be captured
 */
export function shouldCaptureToolResult(toolName: string): boolean {
  return !SKIP_RESULT_TOOLS.includes(toolName);
}

/**
 * Filter tool result for storage
 */
export function filterToolResult(
  toolName: string,
  result: unknown,
  maxLength: number = MAX_RESULT_LENGTH,
): string | null {
  if (!shouldCaptureToolResult(toolName)) {
    return '[Result not captured]';
  }

  if (result === undefined || result === null) {
    return null;
  }

  let resultStr: string;
  if (typeof result === 'string') {
    resultStr = result;
  } else {
    try {
      resultStr = JSON.stringify(result, null, 2);
    } catch {
      resultStr = String(result);
    }
  }

  // Apply filtering
  return filterContent(resultStr, { maxLength, maskSecrets: true });
}

/**
 * Create a redaction reason string
 */
export function createRedactionReason(reason: string): string {
  return `[Content redacted: ${reason}]`;
}
