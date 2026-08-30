import hljs from 'highlight.js/lib/core';
import markdown from 'highlight.js/lib/languages/markdown';

hljs.registerLanguage('markdown', markdown);

/**
 * Paint `fromDoc` markdown as source. Escapes HTML; wraps tokens in spans.
 * Not a preview (headings stay `# …` in the decoded text).
 */
export function highlight(md) {
  return hljs.highlight(md, { language: 'markdown' }).value;
}
