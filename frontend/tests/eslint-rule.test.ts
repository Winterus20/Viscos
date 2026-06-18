/**
 * ESLint custom rule tests — verify the hashed class selector rule.
 */

import { describe, it, expect } from 'vitest';
import { RuleTester } from 'eslint';
import rule from '../eslint-rules/no-hashed-class-selector.js';

const ruleTester = new RuleTester({
  languageOptions: {
    ecmaVersion: 2022,
    sourceType: 'module',
  },
});

describe('no-hashed-class-selector rule', () => {
  it('flags Discord hashed class selectors', () => {
    ruleTester.run('no-hashed-class-selector', rule, {
      valid: [
        // ARIA selectors — OK.
        'document.querySelector(\'[aria-label*="channel"]\')',
        // id prefix — OK.
        'document.querySelector(\'[id^="message-content-"]\')',
        // data-* — OK.
        "document.querySelector('[data-list-item-id]')",
        // Webpack module proxy — OK.
        "viscos.webpack.findByProps('getCurrentUser')",
      ],
      invalid: [
        {
          code: "document.querySelector('.message__5126c')",
          errors: [{ messageId: 'hashedClass' }],
        },
        {
          code: "document.querySelector('.username_c19a55')",
          errors: [{ messageId: 'hashedClass' }],
        },
        {
          code: 'document.querySelector(".channel_c2b9f1")',
          errors: [{ messageId: 'hashedClass' }],
        },
      ],
    });
    expect(true).toBe(true);
  });
});
