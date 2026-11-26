// The mod page screenshot gallery (#115): main image, roving-tabindex thumbnails, the lightbox,
// lazy images with a skeleton, and broken images that drop out.
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { I18nProvider } from '../i18n';
import { ScreenshotGallery } from '../ScreenshotGallery';

const shots = ['https://img.test/1.png', 'https://img.test/2.png', 'https://img.test/3.png'];

function setup(screenshots: string[] = shots) {
  const view = render(
    <I18nProvider locale="en">
      <ScreenshotGallery screenshots={screenshots} title="Rebalanced AI" />
    </I18nProvider>,
  );
  return { ...view, user: userEvent.setup() };
}

const main = () => screen.getByRole('button', { name: /^Rebalanced AI, screenshot \d of \d$/ });
const mainImage = () => main().querySelector('img') as HTMLImageElement;
const thumbs = () => within(screen.getByRole('group', { name: 'Screenshot thumbnails' })).getAllByRole('button');

afterEach(cleanup);

describe('ScreenshotGallery', () => {
  it('renders nothing without screenshots', () => {
    const { container } = setup([]);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the first screenshot as the main image with lazy, private loading', () => {
    setup();
    expect(screen.getByRole('region', { name: 'Screenshots' })).toBeInTheDocument();
    expect(main()).toHaveAccessibleName('Rebalanced AI, screenshot 1 of 3');
    const image = mainImage();
    expect(image).toHaveAttribute('src', shots[0]);
    expect(image).toHaveAttribute('loading', 'lazy');
    expect(image).toHaveAttribute('decoding', 'async');
    expect(image).toHaveAttribute('referrerpolicy', 'no-referrer');
    for (const thumb of thumbs()) {
      const img = thumb.querySelector('img') as HTMLImageElement;
      expect(img).toHaveAttribute('loading', 'lazy');
      expect(img).toHaveAttribute('referrerpolicy', 'no-referrer');
    }
  });

  it('shows a skeleton in a fixed box until the image loads', () => {
    setup();
    const stage = main();
    expect(stage).toHaveAttribute('data-loading', 'true');
    fireEvent.load(mainImage());
    expect(stage).not.toHaveAttribute('data-loading');
  });

  it('moves through the thumbnails with a roving tab stop', async () => {
    const { user } = setup();
    const [first, second, third] = thumbs();
    expect(thumbs().map((thumb) => thumb.tabIndex)).toEqual([0, -1, -1]);
    expect(first).toHaveAttribute('aria-pressed', 'true');
    expect(first).toHaveAccessibleName('Show screenshot 1 of 3');

    first?.focus();
    await user.keyboard('{ArrowRight}');
    expect(second).toHaveFocus();
    expect(second).toHaveAttribute('aria-pressed', 'true');
    expect(first).toHaveAttribute('aria-pressed', 'false');
    expect(thumbs().map((thumb) => thumb.tabIndex)).toEqual([-1, 0, -1]);
    expect(mainImage()).toHaveAttribute('src', shots[1]);

    await user.keyboard('{End}');
    expect(third).toHaveFocus();
    await user.keyboard('{ArrowRight}');
    expect(first).toHaveFocus();
    await user.keyboard('{ArrowLeft}');
    expect(third).toHaveFocus();
    await user.keyboard('{Home}');
    expect(first).toHaveFocus();

    await user.click(third as HTMLElement);
    expect(mainImage()).toHaveAttribute('src', shots[2]);
  });

  it('opens a lightbox that traps focus, navigates with the arrows and closes on Escape', async () => {
    const { user } = setup();
    main().focus();
    await user.keyboard('{Enter}');
    const dialog = screen.getByRole('dialog', { name: 'Rebalanced AI screenshots' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(within(dialog).getByText('1 / 3')).toBeInTheDocument();
    const close = within(dialog).getByRole('button', { name: 'Close viewer' });
    expect(close).toHaveFocus();

    await user.keyboard('{ArrowRight}');
    expect(within(dialog).getByText('2 / 3')).toBeInTheDocument();
    expect(within(dialog).getByRole('img')).toHaveAttribute('src', shots[1]);
    await user.keyboard('{ArrowLeft}{ArrowLeft}');
    expect(within(dialog).getByText('3 / 3')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Next screenshot' }));
    expect(within(dialog).getByText('1 / 3')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Previous screenshot' }));
    expect(within(dialog).getByText('3 / 3')).toBeInTheDocument();

    // Tab cycles inside the dialog.
    const buttons = within(dialog).getAllByRole('button');
    buttons[buttons.length - 1]?.focus();
    await user.tab();
    expect(buttons[0]).toHaveFocus();
    await user.tab({ shift: true });
    expect(buttons[buttons.length - 1]).toHaveFocus();

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(main()).toHaveFocus();
    // The gallery follows the screenshot the viewer ended on.
    expect(mainImage()).toHaveAttribute('src', shots[2]);
  });

  it('opens the lightbox on click and closes it from its button', async () => {
    const { user } = setup();
    await user.click(main());
    await user.click(screen.getByRole('button', { name: 'Close viewer' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows no previous or next buttons for a single screenshot', async () => {
    const { user } = setup([shots[0] as string]);
    await user.click(main());
    expect(screen.queryByRole('button', { name: 'Next screenshot' })).not.toBeInTheDocument();
    expect(screen.getByText('1 / 1')).toBeInTheDocument();
  });

  it('drops a broken screenshot and falls back to the next one', () => {
    setup();
    fireEvent.error(mainImage());
    expect(thumbs()).toHaveLength(2);
    expect(mainImage()).toHaveAttribute('src', shots[1]);
    expect(main()).toHaveAccessibleName('Rebalanced AI, screenshot 1 of 2');
  });

  it('drops a broken thumbnail that is not selected', () => {
    setup();
    fireEvent.error(thumbs()[2]?.querySelector('img') as HTMLImageElement);
    expect(thumbs()).toHaveLength(2);
    expect(mainImage()).toHaveAttribute('src', shots[0]);
  });

  it('collapses when every screenshot fails', () => {
    const { container } = setup(shots.slice(0, 2));
    fireEvent.error(mainImage());
    // One screenshot left: nothing to choose from, so the thumbnail strip goes away.
    expect(screen.queryByRole('group', { name: 'Screenshot thumbnails' })).not.toBeInTheDocument();
    expect(mainImage()).toHaveAttribute('src', shots[1]);
    fireEvent.error(mainImage());
    expect(container).toBeEmptyDOMElement();
  });
});
