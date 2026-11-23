export type BannerVariant = 'info' | 'success' | 'warning' | 'error';

// Decorative 16x16 stroke glyphs, one per banner variant (inline SVG, no icon library).
const GLYPHS: Record<BannerVariant, string[]> = {
  info: ['M8 7.2V11', 'M8 4.8v.1'],
  success: ['M5 8.4l2.1 2.1L11 6'],
  warning: ['M8 4.6v4', 'M8 11v.1'],
  error: ['M5.6 5.6l4.8 4.8', 'M10.4 5.6l-4.8 4.8'],
};

const SHAPES: Record<BannerVariant, string> = {
  info: 'M8 1.8a6.2 6.2 0 1 0 0 12.4A6.2 6.2 0 0 0 8 1.8z',
  success: 'M8 1.8a6.2 6.2 0 1 0 0 12.4A6.2 6.2 0 0 0 8 1.8z',
  warning: 'M8 2L14.5 13.5H1.5z',
  error: 'M8 1.8a6.2 6.2 0 1 0 0 12.4A6.2 6.2 0 0 0 8 1.8z',
};

export function BannerIcon({ variant }: { variant: BannerVariant }) {
  return (
    <svg className="banner-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d={SHAPES[variant]} />
      {GLYPHS[variant].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
