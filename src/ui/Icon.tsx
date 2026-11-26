import type { ReactNode } from 'react';

// Decorative 20x20 stroke glyphs drawn with one stroke style (inline SVG, no icon library).
const GLYPHS = {
  library: (
    <>
      <path d="M3.5 6.2L10 3l6.5 3.2v7.6L10 17l-6.5-3.2z" />
      <path d="M3.5 6.2L10 9.4l6.5-3.2M10 9.4V17" />
    </>
  ),
  profiles: (
    <>
      <circle cx="10" cy="6.8" r="3" />
      <path d="M4 16.5c.5-3 3-4.6 6-4.6s5.5 1.6 6 4.6" />
    </>
  ),
  settings: (
    <>
      <path d="M3 6h8M15 6h2M3 14h2M9 14h8" />
      <circle cx="13" cy="6" r="2" />
      <circle cx="7" cy="14" r="2" />
    </>
  ),
  search: (
    <>
      <circle cx="9" cy="9" r="5" />
      <path d="M13 13l3.5 3.5" />
    </>
  ),
  import: (
    <>
      <path d="M10 3.5v9M6.5 9L10 12.5 13.5 9" />
      <path d="M4 13.5v2.5h12v-2.5" />
    </>
  ),
  play: <path d="M7 4.8v10.4l8.2-5.2z" />,
  stop: <rect x="5.5" y="5.5" width="9" height="9" rx="1.5" />,
  game: (
    <>
      <rect x="2.5" y="6" width="15" height="9" rx="3" />
      <path d="M6.5 9v3M5 10.5h3M12.5 10h.01M14.5 11.5h.01" />
    </>
  ),
  folder: <path d="M3 5.5h4.5l1.5 1.5H17v8.5H3z" />,
  refresh: (
    <>
      <path d="M15.5 8A5.8 5.8 0 0 0 5 6.2M4.5 12A5.8 5.8 0 0 0 15 13.8" />
      <path d="M5 3.5v2.8h2.8M15 16.5v-2.8h-2.8" />
    </>
  ),
  language: (
    <>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M3.5 10h13M10 3.5c2 2 2.8 4.2 2.8 6.5s-.8 4.5-2.8 6.5c-2-2-2.8-4.2-2.8-6.5S8 5.5 10 3.5z" />
    </>
  ),
  filter: <path d="M3.5 5h13l-5 6v4.5l-3-1.5v-3z" />,
  up: <path d="M5.5 12.5L10 8l4.5 4.5" />,
  down: <path d="M5.5 7.5L10 12l4.5-4.5" />,
  'chevron-left': <path d="M12.5 5.5L8 10l4.5 4.5" />,
  'chevron-right': <path d="M7.5 5.5L12 10l-4.5 4.5" />,
  plus: <path d="M10 4.5v11M4.5 10h11" />,
  check: <path d="M4.5 10.5l3.5 3.5 7.5-8" />,
  info: (
    <>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M10 9v4.5M10 6.5h.01" />
    </>
  ),
  storage: (
    <>
      <rect x="3" y="4.5" width="14" height="11" rx="2" />
      <path d="M3 11h14M6.5 13.25h.01M9 13.25h.01" />
    </>
  ),
  // Discover tab (#73): a compass, the grid/list view toggle, an external page and a close cross.
  discover: (
    <>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M12.8 7.2l-1.6 4-4 1.6 1.6-4z" />
    </>
  ),
  grid: (
    <>
      <rect x="3.5" y="3.5" width="5" height="5" rx="1" />
      <rect x="11.5" y="3.5" width="5" height="5" rx="1" />
      <rect x="3.5" y="11.5" width="5" height="5" rx="1" />
      <rect x="11.5" y="11.5" width="5" height="5" rx="1" />
    </>
  ),
  list: <path d="M7.5 5.5h9M7.5 10h9M7.5 14.5h9M3.5 5.5h.01M3.5 10h.01M3.5 14.5h.01" />,
  external: (
    <>
      <path d="M11 3.5h5.5V9M16.5 3.5L9.5 10.5" />
      <path d="M14 11.5v4H4.5V6h4" />
    </>
  ),
  close: <path d="M5.5 5.5l9 9M14.5 5.5l-9 9" />,
  // Community account (#122): a generic person (no third-party brand mark).
  account: (
    <>
      <circle cx="10" cy="7" r="3" />
      <path d="M4.5 16c.8-2.9 3-4.5 5.5-4.5s4.7 1.6 5.5 4.5" />
    </>
  ),
  // Support the project (#94): an outlined heart, a generic glyph (no third-party brand mark).
  heart: (
    <path d="M10 16c-3.8-2.6-6.5-5.1-6.5-8 0-2 1.5-3.5 3.4-3.5 1.3 0 2.4.7 3.1 1.8.7-1.1 1.8-1.8 3.1-1.8 1.9 0 3.4 1.5 3.4 3.5 0 2.9-2.7 5.4-6.5 8z" />
  ),
  // Community stars (#119): a five-point star; CSS fills it when the mod is starred.
  star: <path d="M10 3l2.1 4.4 4.8.6-3.5 3.3.9 4.8L10 13.8l-4.3 2.3.9-4.8L3.1 8l4.8-.6z" />,
  // Sponsored card without an image (#123): a generic megaphone on the icon tile.
  megaphone: (
    <>
      <path d="M3.5 8v4h2.5l7 3.5v-11l-7 3.5z" />
      <path d="M6 12l1 3.5h2l-.7-2.6" />
      <path d="M15.5 8.2a2.2 2.2 0 010 3.6" />
    </>
  ),
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof GLYPHS;

export function Icon({ name, className = 'icon' }: { name: IconName; className?: string }) {
  return (
    <svg className={className} viewBox="0 0 20 20" aria-hidden="true" focusable="false">
      {GLYPHS[name]}
    </svg>
  );
}

/** The launcher mark (shown in the title bar), on the same 20x20 grid as the icons. */
export function BrandMark({
  className = 'titlebar-mark',
  ...rest
}: {
  className?: string;
  'data-tauri-drag-region'?: string;
}) {
  return (
    <svg className={className} viewBox="0 0 20 20" aria-hidden="true" focusable="false" {...rest}>
      <rect x="2" y="2" width="16" height="16" rx="4" />
      <path d="M6 13.5l2.6-7 1.4 3.6 1.4-3.6 2.6 7" />
    </svg>
  );
}

// Window-control glyphs drawn like the Windows 11 caption buttons: a 10x10 grid, 1px square strokes.
const WINDOW_GLYPHS = {
  minimize: <path d="M0 5.5h10" />,
  maximize: <rect x="0.5" y="0.5" width="9" height="9" />,
  restore: (
    <>
      <path d="M2.5 2.5V0.5h7v7h-2" />
      <rect x="0.5" y="2.5" width="7" height="7" />
    </>
  ),
  close: <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" />,
} satisfies Record<string, ReactNode>;

export type WindowGlyphName = keyof typeof WINDOW_GLYPHS;

export function WindowGlyph({ name }: { name: WindowGlyphName }) {
  return (
    <svg className="titlebar-glyph" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
      {WINDOW_GLYPHS[name]}
    </svg>
  );
}
