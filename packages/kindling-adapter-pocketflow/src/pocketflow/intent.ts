/**
 * Intent Inference
 *
 * Derives semantic intent from node names using pattern matching.
 * Intent categorizes what a node is trying to accomplish, enabling
 * smarter retrieval and context organization.
 */

/**
 * Mapping from keyword patterns to intent strings
 */
export interface IntentMapping {
  /** Keywords that indicate this intent (matched against node name) */
  keywords: string[];
  /** The intent string to return when matched */
  intent: string;
}

/**
 * Default intent patterns based on common development workflows
 */
export const DEFAULT_INTENT_PATTERNS: IntentMapping[] = [
  // Testing
  { keywords: ['test', 'spec', 'check', 'verify', 'validate', 'assert'], intent: 'test' },

  // Building/Compilation
  { keywords: ['build', 'compile', 'bundle', 'pack', 'transpile'], intent: 'build' },

  // Deployment
  { keywords: ['deploy', 'publish', 'release', 'ship', 'push-to'], intent: 'deploy' },

  // Debugging/Fixing
  { keywords: ['fix', 'debug', 'repair', 'patch', 'hotfix', 'troubleshoot'], intent: 'debug' },

  // Feature implementation
  { keywords: ['implement', 'add', 'create', 'feature', 'develop', 'make'], intent: 'feature' },

  // Refactoring
  {
    keywords: ['refactor', 'restructure', 'reorganize', 'cleanup', 'clean-up'],
    intent: 'refactor',
  },

  // Data processing
  {
    keywords: ['process', 'transform', 'convert', 'parse', 'extract', 'load', 'etl'],
    intent: 'process',
  },

  // Analysis/Research
  {
    keywords: ['analyze', 'analyse', 'research', 'investigate', 'explore', 'scan'],
    intent: 'analyze',
  },

  // Generation
  { keywords: ['generate', 'gen', 'scaffold', 'template', 'init', 'setup'], intent: 'generate' },

  // Communication/API
  {
    keywords: ['fetch', 'request', 'call', 'api', 'http', 'send', 'receive'],
    intent: 'communicate',
  },

  // Storage/Persistence
  { keywords: ['save', 'store', 'persist', 'write', 'cache', 'backup'], intent: 'store' },

  // Retrieval
  { keywords: ['read', 'get', 'fetch', 'load', 'retrieve', 'query'], intent: 'retrieve' },

  // Cleanup/Maintenance
  { keywords: ['clean', 'clear', 'remove', 'delete', 'prune', 'gc'], intent: 'cleanup' },

  // Monitoring/Logging
  { keywords: ['log', 'monitor', 'track', 'metric', 'observe', 'report'], intent: 'monitor' },
];

/**
 * Normalizes a node name for pattern matching.
 * Converts camelCase, PascalCase, snake_case, and kebab-case to space-separated words.
 */
function normalizeNodeName(name: string): string {
  return (
    name
      // Insert space before uppercase letters (camelCase/PascalCase)
      .replace(/([a-z])([A-Z])/g, '$1 $2')
      // Replace underscores and hyphens with spaces
      .replace(/[_-]/g, ' ')
      // Convert to lowercase
      .toLowerCase()
      // Collapse multiple spaces
      .replace(/\s+/g, ' ')
      .trim()
  );
}

/**
 * Infers intent from a node name using pattern matching.
 *
 * @param nodeName - The name of the node (e.g., "run-tests", "deployProduction", "fix_auth_bug")
 * @param patterns - Custom patterns to use (defaults to DEFAULT_INTENT_PATTERNS)
 * @returns The inferred intent string, or "general" if no pattern matches
 *
 * @example
 * ```typescript
 * inferIntent('run-tests')           // → 'test'
 * inferIntent('buildApp')            // → 'build'
 * inferIntent('deploy_production')   // → 'deploy'
 * inferIntent('fixAuthBug')          // → 'debug'
 * inferIntent('implementFeatureX')   // → 'feature'
 * inferIntent('unknownNode')         // → 'general'
 * ```
 */
export function inferIntent(
  nodeName: string,
  patterns: IntentMapping[] = DEFAULT_INTENT_PATTERNS,
): string {
  const normalized = normalizeNodeName(nodeName);

  // Handle empty or whitespace-only input
  if (!normalized) {
    return 'general';
  }

  const words = normalized.split(' ').filter((w) => w.length > 0);

  // Check each pattern's keywords against the normalized words
  for (const pattern of patterns) {
    for (const keyword of pattern.keywords) {
      // Check if any word starts with the keyword (prefix match)
      // This handles cases like "testing" matching "test"
      // Require minimum word length of 2 to avoid false positives
      if (
        words.some(
          (word) => word.length >= 2 && (word.startsWith(keyword) || keyword.startsWith(word)),
        )
      ) {
        return pattern.intent;
      }
    }
  }

  return 'general';
}
