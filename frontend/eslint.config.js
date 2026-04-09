import config from '@sveltejs/eslint-config';
import svelte from 'eslint-plugin-svelte';

export default [
  {
    ignores: ['build/', 'dist/', 'node_modules/', 'wailsjs/'],
  },
  ...config,
  {
    languageOptions: {
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    files: ['**/*.ts', '**/*.svelte'],
  },
  {
    plugins: {
      svelte,
    },
  },
  {
    rules: {
      'no-unused-vars': ['warn', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    }
  }
];
