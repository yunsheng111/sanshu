/// <reference types="vitest" />
import { resolve } from 'node:path'
import Vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [Vue()],
  test: {
    globals: true,
    environment: 'happy-dom',
    include: ['src/frontend/**/*.{test,spec}.{ts,tsx}'],
    setupFiles: ['./vitest.setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: ['src/frontend/**/*.{ts,vue}'],
      exclude: [
        'src/frontend/types/**',
        'src/frontend/**/*.d.ts',
        'src/frontend/test/**',
      ],
    },
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src/frontend'),
    },
  },
})
