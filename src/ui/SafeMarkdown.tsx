import { Fragment } from 'react';
import type { ReactNode } from 'react';
import { parseSafeMarkdown } from './markdownSubset';
import type { Inline } from './markdownSubset';

/** The page title is the h2, so `#` renders as h3, `##` as h4 and `###` as h5. */
const HEADINGS = { 1: 'h3', 2: 'h4', 3: 'h5' } as const;

function renderInline(runs: Inline[]): ReactNode {
  return runs.map((run, index) => {
    if (run.kind === 'text') return <Fragment key={index}>{run.text}</Fragment>;
    if (run.kind === 'strong') return <strong key={index}>{renderInline(run.children)}</strong>;
    return <em key={index}>{renderInline(run.children)}</em>;
  });
}

/**
 * Renders a mod description in the safe Markdown subset (#115) as React elements only. There is
 * no HTML path: raw tags, links and images reach the screen as escaped text, and no anchor is made.
 */
export function SafeMarkdown({ source }: { source: string }) {
  return (
    <div className="markdown">
      {parseSafeMarkdown(source).map((block, index) => {
        if (block.kind === 'heading') {
          const Heading = HEADINGS[block.level];
          return <Heading key={index}>{renderInline(block.content)}</Heading>;
        }
        if (block.kind === 'list') {
          const List = block.ordered ? 'ol' : 'ul';
          return (
            <List key={index}>
              {block.items.map((item, itemIndex) => (
                <li key={itemIndex}>{renderInline(item)}</li>
              ))}
            </List>
          );
        }
        return <p key={index}>{renderInline(block.content)}</p>;
      })}
    </div>
  );
}
