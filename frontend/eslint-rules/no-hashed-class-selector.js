/**
 * Custom ESLint rule: forbid hashed class selectors.
 *
 * Detects patterns like:
 *   document.querySelector('.message__5126c')
 *   document.querySelector(".username_c19a55")
 *
 * Why: Discord ships webpack-hashed class names that change on every deploy.
 *       Our bridge.ts should use aria-label / role / data-* / id-prefix
 *       selectors instead. See:
 *       crates/viscos-webview/BRIDGE-RESILIENCE.md §2.2
 */
export default {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Forbid Discord hashed class selectors (e.g. `.message__5126c`) in bridge.ts — they break every Discord deploy.',
    },
    schema: [],
    messages: {
      hashedClass:
        "Hashed class selector '{{selector}}' is forbidden. {{suggestion}} See crates/viscos-webview/BRIDGE-RESILIENCE.md §2.2.",
    },
  },
  create(context) {
    function checkCall(node) {
      if (
        node.callee.type === 'MemberExpression' &&
        node.callee.property &&
        node.callee.property.name === 'querySelector' &&
        node.arguments[0] &&
        node.arguments[0].type === 'Literal' &&
        typeof node.arguments[0].value === 'string'
      ) {
        const sel = node.arguments[0].value;
        // Pattern: .classname_xxxxx where xxxxx is a 4+ char hex hash.
        if (/\.[a-zA-Z]+_[a-f0-9]{4,}/.test(sel)) {
          context.report({
            node: node.arguments[0],
            messageId: 'hashedClass',
            data: {
              selector: sel,
              suggestion:
                'Use [aria-label*="..."], [role="..."], [data-*] or viscos.webpack.findByProps(...) instead.',
            },
          });
        }
      }
    }
    return {
      CallExpression: checkCall,
    };
  },
};
