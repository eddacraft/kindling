import { defineConfig } from 'vitest/config';
import { resolve } from 'path';

export default defineConfig({
  test: {
    globals: true,
    testTimeout: 20000,
    hookTimeout: 20000,
  },
  resolve: {
    alias: {
      '@eddacraft/kindling': resolve(__dirname, '../kindling/src/index.ts'),
    },
  },
});
