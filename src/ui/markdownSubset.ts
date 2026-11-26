/**
 * The safe Markdown subset of mod descriptions (#115), as a pure parser. It knows paragraphs,
 * `#`/`##`/`###` headings, `**bold**`, `*italic*`, `-`/`*` lists and `1.` lists. Everything else,
 * HTML tags, links and images included, stays literal text: the result holds only strings, and
 * `SafeMarkdown` turns it into React elements (React escapes the text), never into HTML.
 */

export type Inline =
  { kind: 'text'; text: string } | { kind: 'strong'; children: Inline[] } | { kind: 'em'; children: Inline[] };

export type Block =
  | { kind: 'paragraph'; content: Inline[] }
  | { kind: 'heading'; level: 1 | 2 | 3; content: Inline[] }
  | { kind: 'list'; ordered: boolean; items: Inline[][] };

const HEADING = /^(#{1,3})\s+(.*\S)\s*$/;
const BULLET = /^\s*[-*]\s+(.*\S)\s*$/;
const NUMBERED = /^\s*\d{1,9}\.\s+(.*\S)\s*$/;

const isSpace = (char: string | undefined) => char !== undefined && /\s/.test(char);

/** The next single `*` from `from`, skipping `**` pairs; -1 when there is none. */
function closingStar(text: string, from: number): number {
  let index = from;
  while (index < text.length) {
    if (text.startsWith('**', index)) {
      index += 2;
    } else if (text[index] === '*') {
      return index;
    } else {
      index += 1;
    }
  }
  return -1;
}

/** Splits a line into text, bold and italic runs. Unmatched or spaced markers stay as text. */
export function parseInline(text: string): Inline[] {
  const runs: Inline[] = [];
  let buffer = '';
  const flush = () => {
    if (buffer !== '') runs.push({ kind: 'text', text: buffer });
    buffer = '';
  };
  // A marker opens only before a non-space and closes only after one, like CommonMark emphasis.
  const valid = (inner: string) => inner !== '' && !isSpace(inner[0]) && !isSpace(inner[inner.length - 1]);

  let index = 0;
  while (index < text.length) {
    if (text.startsWith('**', index)) {
      const end = text.indexOf('**', index + 2);
      const inner = end === -1 ? '' : text.slice(index + 2, end);
      if (valid(inner)) {
        flush();
        runs.push({ kind: 'strong', children: parseInline(inner) });
        index = end + 2;
        continue;
      }
    } else if (text[index] === '*') {
      const end = closingStar(text, index + 1);
      const inner = end === -1 ? '' : text.slice(index + 1, end);
      if (valid(inner)) {
        flush();
        runs.push({ kind: 'em', children: parseInline(inner) });
        index = end + 1;
        continue;
      }
    }
    buffer += text[index];
    index += 1;
  }
  flush();
  return runs;
}

/** Parses the description into blocks. Blank lines separate paragraphs; a line break inside one is kept. */
export function parseSafeMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: Inline[][] } | null = null;

  const endParagraph = () => {
    if (paragraph.length > 0) blocks.push({ kind: 'paragraph', content: parseInline(paragraph.join('\n')) });
    paragraph = [];
  };
  const endList = () => {
    if (list !== null) blocks.push({ kind: 'list', ...list });
    list = null;
  };
  const addItem = (ordered: boolean, text: string) => {
    endParagraph();
    if (list !== null && list.ordered !== ordered) endList();
    list ??= { ordered, items: [] };
    list.items.push(parseInline(text));
  };

  for (const raw of source.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (line.trim() === '') {
      endParagraph();
      endList();
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading !== null) {
      endParagraph();
      endList();
      const level = (heading[1] ?? '#').length as 1 | 2 | 3;
      blocks.push({ kind: 'heading', level, content: parseInline(heading[2] ?? '') });
      continue;
    }
    const bullet = BULLET.exec(line);
    if (bullet !== null) {
      addItem(false, bullet[1] ?? '');
      continue;
    }
    const numbered = NUMBERED.exec(line);
    if (numbered !== null) {
      addItem(true, numbered[1] ?? '');
      continue;
    }
    endList();
    paragraph.push(line.trim());
  }
  endParagraph();
  endList();
  return blocks;
}
