import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    // Daemon spawn + UDS round-trips need more than the 5s default.
    testTimeout: 20000,
    hookTimeout: 20000,
  },
});
