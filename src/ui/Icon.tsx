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
  plus: <path d="M10 4.5v11M4.5 10h11" />,
  check: <path d="M4.5 10.5l3.5 3.5 7.5-8" />,
  info: (
    <>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M10 9v4.5M10 6.5h.01" />
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
