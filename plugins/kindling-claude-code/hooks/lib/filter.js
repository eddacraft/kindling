/**
 * Content filtering for Kindling plugin
 *
 * Prevents accidental capture of secrets and reduces noise.
 */

// Maximum content length before truncation
const MAX_CONTENT_LENGTH = 10000;

// Secret patterns to detect
const SECRET_PATTERNS = [
  /['\"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['\"]?\s*[:=]\s*['\"]?([^\s'\"]{8,})['\"]?/gi,
  /sk-ant-[A-Za-z0-9\-_]{20,}/g, // Anthropic keys
  /sk-[A-Za-z0-9]{40,}/g, // OpenAI keys
  /Bearer\s+[A-Za-z0-9\-._~+/]{20,}/gi,
  /Basic\s+[A-Za-z0-9+/]{20,}/gi,
];

// Tools that should be skipped entirely (too noisy)
const SKIP_TOOLS = ['WebSearch'];

// Tools whose results should be truncated more aggressively
const NOISY_TOOLS = ['Grep', 'Glob'];

/**
 * Truncate content if too long
 */
export function truncate(content, maxLength = MAX_CONTENT_LENGTH) {
  if (!content || content.length <= maxLength) {
    return content;
  }
  return content.substring(0, maxLength) + `\n[Truncated ${content.length - maxLength} chars]`;
}

/**
 * Check if content likely contains secrets
 */
export function containsSecrets(content) {
  if (!content) return false;
  return SECRET_PATTERNS.some((pattern) => {
    pattern.lastIndex = 0;
    return pattern.test(content);
  });
}

/**
 * Mask secrets in content
 */
export function maskSecrets(content) {
  if (!content) return content;
  let masked = content;

  for (const pattern of SECRET_PATTERNS) {
    pattern.lastIndex = 0;
    masked = masked.replace(pattern, (match) => {
      if (match.includes('=') || match.includes(':')) {
        const parts = match.split(/[:=]/);
        return `${parts[0]}=[REDACTED]`;
      }
      return '[REDACTED]';
    });
  }

  return masked;
}

/**
 * Filter content for storage
 */
export function filterContent(content) {
  if (!content) return content;

  let filtered = content;

  // Mask secrets
  if (containsSecrets(filtered)) {
    filtered = maskSecrets(filtered);
  }

  // Truncate
  filtered = truncate(filtered);

  return filtered;
}

/**
 * Check if tool result should be captured
 */
export function shouldCaptureTool(toolName) {
  // Skip tools that produce too much noise
  if (SKIP_TOOLS.includes(toolName)) return false;
  return true;
}

/**
 * Check if tool is noisy (results should be truncated more)
 */
export function isNoisyTool(toolName) {
  return NOISY_TOOLS.includes(toolName);
}

/**
 * Filter tool result for storage
 */
export function filterToolResult(toolName, result) {
  if (!shouldCaptureTool(toolName)) {
    return '[Result not captured]';
  }

  if (result === undefined || result === null) {
    return null;
  }

  let resultStr;
  if (typeof result === 'string') {
    resultStr = result;
  } else {
    try {
      resultStr = JSON.stringify(result);
    } catch {
      resultStr = String(result);
    }
  }

  // Shorter limit for noisy tools
  const maxLen = isNoisyTool(toolName) ? 2000 : MAX_CONTENT_LENGTH;
  return truncate(maskSecrets(resultStr), maxLen);
}
