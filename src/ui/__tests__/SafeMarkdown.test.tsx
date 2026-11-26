// The safe Markdown subset used by mod descriptions (#115): a pure parser into blocks and inline
// runs, rendered as React elements only. Anything outside the subset (HTML, links, images, code)
// stays literal text.
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SafeMarkdown } from '../SafeMarkdown';
import { parseInline, parseSafeMarkdown } from '../markdownSubset';

afterEach(cleanup);

describe('parseSafeMarkdown', () => {
  it('returns no blocks for empty or blank text', () => {
    expect(parseSafeMarkdown('')).toEqual([]);
    expect(parseSafeMarkdown('  \n\n \r\n')).toEqual([]);
  });

  it('splits paragraphs on blank lines and keeps single line breaks inside a paragraph', () => {
    expect(parseSafeMarkdown('First line\nsecond line\n\nNext paragraph')).toEqual([
      { kind: 'paragraph', content: [{ kind: 'text', text: 'First line\nsecond line' }] },
      { kind: 'paragraph', content: [{ kind: 'text', text: 'Next paragraph' }] },
    ]);
  });

  it('reads Windows line endings', () => {
    expect(parseSafeMarkdown('One\r\n\r\nTwo')).toHaveLength(2);
  });

  it('parses three heading levels and leaves deeper ones as text', () => {
    expect(parseSafeMarkdown('# One\n## Two\n### Three\n#### Four')).toEqual([
      { kind: 'heading', level: 1, content: [{ kind: 'text', text: 'One' }] },
      { kind: 'heading', level: 2, content: [{ kind: 'text', text: 'Two' }] },
      { kind: 'heading', level: 3, content: [{ kind: 'text', text: 'Three' }] },
      { kind: 'paragraph', content: [{ kind: 'text', text: '#### Four' }] },
    ]);
  });

  it('needs a space after the hashes', () => {
    expect(parseSafeMarkdown('#hashtag')).toEqual([
      { kind: 'paragraph', content: [{ kind: 'text', text: '#hashtag' }] },
    ]);
  });

  it('parses unordered lists with - or * and ordered lists with 1.', () => {
    expect(parseSafeMarkdown('- a\n* b\n\n1. one\n2. two')).toEqual([
      { kind: 'list', ordered: false, items: [[{ kind: 'text', text: 'a' }], [{ kind: 'text', text: 'b' }]] },
      { kind: 'list', ordered: true, items: [[{ kind: 'text', text: 'one' }], [{ kind: 'text', text: 'two' }]] },
    ]);
  });

  it('starts a new list when the list kind changes and ends a paragraph at a list or heading', () => {
    const blocks = parseSafeMarkdown('Intro\n- a\n1. b\n# Title');
    expect(blocks.map((block) => block.kind)).toEqual(['paragraph', 'list', 'list', 'heading']);
  });

  it('keeps raw HTML as literal text', () => {
    expect(parseSafeMarkdown('<script>alert(1)</script> <b>x</b>')).toEqual([
      { kind: 'paragraph', content: [{ kind: 'text', text: '<script>alert(1)</script> <b>x</b>' }] },
    ]);
  });

  it('keeps links and images as literal text', () => {
    const text = '[site](https://example.com) ![img](https://example.com/a.png)';
    expect(parseSafeMarkdown(text)).toEqual([{ kind: 'paragraph', content: [{ kind: 'text', text }] }]);
  });
});

describe('parseInline', () => {
  it('parses bold and italic', () => {
    expect(parseInline('a **bold** and *italic* b')).toEqual([
      { kind: 'text', text: 'a ' },
      { kind: 'strong', children: [{ kind: 'text', text: 'bold' }] },
      { kind: 'text', text: ' and ' },
      { kind: 'em', children: [{ kind: 'text', text: 'italic' }] },
      { kind: 'text', text: ' b' },
    ]);
  });

  it('nests italic inside bold and bold inside italic', () => {
    expect(parseInline('**a *b* c**')).toEqual([
      {
        kind: 'strong',
        children: [
          { kind: 'text', text: 'a ' },
          { kind: 'em', children: [{ kind: 'text', text: 'b' }] },
          { kind: 'text', text: ' c' },
        ],
      },
    ]);
    expect(parseInline('*a **b** c*')).toEqual([
      {
        kind: 'em',
        children: [
          { kind: 'text', text: 'a ' },
          { kind: 'strong', children: [{ kind: 'text', text: 'b' }] },
          { kind: 'text', text: ' c' },
        ],
      },
    ]);
  });

  it('leaves unmatched or spaced markers as text', () => {
    expect(parseInline('2 * 3 * 4')).toEqual([{ kind: 'text', text: '2 * 3 * 4' }]);
    expect(parseInline('**open')).toEqual([{ kind: 'text', text: '**open' }]);
    expect(parseInline('a *b')).toEqual([{ kind: 'text', text: 'a *b' }]);
    expect(parseInline('a *b * c')).toEqual([{ kind: 'text', text: 'a *b * c' }]);
    expect(parseInline('****')).toEqual([{ kind: 'text', text: '****' }]);
  });

  it('handles a long run of markers without hanging', () => {
    const text = '*'.repeat(20000);
    const started = Date.now();
    const runs = parseInline(text);
    expect(Date.now() - started).toBeLessThan(2000);
    expect(runs.length).toBeGreaterThan(0);
  });
});

describe('SafeMarkdown', () => {
  it('renders headings one level below the page title, bold, italic and lists', () => {
    const { container } = render(
      <SafeMarkdown source={'# Features\n## More\n### Most\n\nThe **best** *mod*.\n\n- one\n- two\n\n1. first'} />,
    );
    expect(screen.getByRole('heading', { level: 3, name: 'Features' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 4, name: 'More' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 5, name: 'Most' })).toBeInTheDocument();
    expect(container.querySelector('strong')).toHaveTextContent('best');
    expect(container.querySelector('em')).toHaveTextContent('mod');
    expect(container.querySelector('ul')?.querySelectorAll('li')).toHaveLength(2);
    expect(container.querySelector('ol')?.querySelectorAll('li')).toHaveLength(1);
  });

  it('shows HTML as text and renders no element from it, and no anchors', () => {
    const { container } = render(
      <SafeMarkdown source={'<img src=x onerror="alert(1)"> <a href="https://x.test">x</a> [y](https://y.test)'} />,
    );
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('a')).toBeNull();
    expect(container.querySelector('script')).toBeNull();
    expect(container).toHaveTextContent(
      '<img src=x onerror="alert(1)"> <a href="https://x.test">x</a> [y](https://y.test)',
    );
  });
});
