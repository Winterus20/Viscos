import tseslint from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import noHashedClassSelector from './eslint-rules/no-hashed-class-selector.js';

/**
 * ESLint v9 flat config — see https://eslint.org/docs/latest/use/configure/configuration-files
 *
 * Faz 1.0: only enforces the no-hashed-class-selector rule (ADR-0012 §2).
 * Phase 1.5+ will tighten with @typescript-eslint/recommended-type-checked.
 */
export default [
  {
    ignores: ['dist/**', 'node_modules/**', 'eslint-rules/**'],
  },
  {
    files: ['src/**/*.ts', 'tests/**/*.ts'],
    languageOptions: {
      parser: tsParser,
      ecmaVersion: 2022,
      sourceType: 'module',
    },
    plugins: {
      '@typescript-eslint': tseslint,
      local: {
        rules: {
          'no-hashed-class-selector': noHashedClassSelector,
        },
      },
    },
    rules: {
      '@typescript-eslint/no-explicit-any': 'warn',
      'no-console': ['error', { allow: ['warn', 'error'] }],
      'local/no-hashed-class-selector': 'error',
    },
  },
];
