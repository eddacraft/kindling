/**
 * Content filtering and safety utilities
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
 * Common secret patterns to detect in content
 */
const SECRET_PATTERNS = [
  // API keys and tokens
  /['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?([^\s'"]+)['"]?/gi,
  // AWS keys
  /(?:AWS|aws)[-_]?(?:SECRET|secret)[-_]?(?:ACCESS|access)[-_]?(?:KEY|key)\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})['"]?/g,
  // Generic API tokens (long alphanumeric strings with mixed chars that look like tokens)
  // Must have at least one digit AND one letter AND be 32+ chars
  /\b(?=.*[0-9])(?=.*[A-Za-z])[A-Za-z0-9]{32,}\b/g,
  // Bearer tokens
  /Bearer\s+([A-Za-z0-9\-._~+/]+=*)/gi,
  // Basic auth
  /Basic\s+([A-Za-z0-9+/]+=*)/gi,
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
 * Options for content filtering
 */
export interface FilterOptions {
  /**
   * Maximum content length before truncation
   */
  maxLength?: number;

  /**
   * Whether to detect and mask secrets
   */
  maskSecrets?: boolean;

  /**
   * Whether to append truncation notice
   */
  showTruncationNotice?: boolean;
}

/**
 * Truncate content if it exceeds max length
 *
 * @param content - Content to truncate
 * @param options - Filter options
 * @returns Truncated content with optional notice
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
 *
 * @param content - Content to check
 * @returns True if content likely contains secrets
 */
export function containsSecrets(content: string): boolean {
  return SECRET_PATTERNS.some((pattern) => {
    pattern.lastIndex = 0; // Reset regex state
    return pattern.test(content);
  });
}

/**
 * Mask potential secrets in content
 *
 * Replaces patterns that look like secrets with [REDACTED]
 *
 * @param content - Content to mask
 * @returns Content with secrets masked
 */
export function maskSecrets(content: string): string {
  let masked = content;

  for (const pattern of SECRET_PATTERNS) {
    pattern.lastIndex = 0; // Reset regex state
    masked = masked.replace(pattern, (match) => {
      // Keep the key name but mask the value
      if (match.includes(':') || match.includes('=')) {
        const parts = match.split(/[:=]/);
        return `${parts[0]}=[REDACTED]`;
      }
      // For bearer/basic tokens and long strings, just redact
      return '[REDACTED]';
    });
  }

  return masked;
}

/**
 * Filter content with all safety rules
 *
 * Applies truncation and optional secret masking
 *
 * @param content - Content to filter
 * @param options - Filter options
 * @returns Filtered content
 */
export function filterContent(content: string, options: FilterOptions = {}): string {
  const { maskSecrets: shouldMask = false } = options;

  let filtered = content;

  // Apply secret masking if enabled
  if (shouldMask && containsSecrets(filtered)) {
    filtered = maskSecrets(filtered);
  }

  // Apply truncation
  filtered = truncateContent(filtered, options);

  return filtered;
}

/**
 * Check if a file path should be excluded from capture
 *
 * @param path - File path to check
 * @returns True if path should be excluded
 */
export function isExcludedPath(path: string): boolean {
  return EXCLUDED_PATHS.some((pattern) => pattern.test(path));
}

/**
 * Filter tool result fields
 *
 * For certain tools, only capture specific fields to reduce noise
 *
 * @param toolName - Name of the tool
 * @param result - Tool result object
 * @returns Filtered result
 */
export function filterToolResult(_toolName: string, result: unknown): unknown {
  // For now, return as-is
  // Future: implement allowlists for specific tools
  // e.g., for 'read_file', only capture first N lines
  // for 'list_dir', limit number of entries
  return result;
}

/**
 * Create a redaction reason string
 *
 * @param reason - Reason for redaction
 * @returns Formatted redaction message
 */
export function createRedactionReason(reason: string): string {
  return `[Content redacted: ${reason}]`;
}
