import { defineConfig } from 'vitest/config';
import { resolve } from 'path';

export default defineConfig({
  test: {
    globals: true,
    // Daemon spawn + UDS round-trips need more than the 5s default.
    testTimeout: 20000,
    hookTimeout: 20000,
  },
  resolve: {
    alias: {
      '@eddacraft/kindling': resolve(__dirname, '../kindling/src/index.ts'),
    },
  },
});
