import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AdPreferenceStore, AdProvider, LauncherState } from '../../application';
import type { AdSlotId, AdViewModel } from '../../domain';
import { AdPreferenceProvider } from '../AdPreferenceContext';
import { AdsContext } from '../AdsContext';
import { AdSlot } from '../AdSlot';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';
import { LauncherContext } from '../LauncherContext';
import { createFakeController } from './fakeController';

const creative: AdViewModel = {
  id: 'c1',
  title: 'Track day tyres',
  body: 'Grip for every lap.',
  imageDataUri: 'data:image/png;base64,AAAA',
  sponsorLabel: 'Acme Racing',
};

function fakeProvider(get: AdProvider['get'] = () => Promise.resolve(creative)) {
  return {
    get: vi.fn(get),
    click: vi.fn(() => Promise.resolve()),
    reportShown: vi.fn(() => Promise.resolve()),
  };
}

function preferenceStore(value: 'on' | 'off'): AdPreferenceStore {
  return { load: () => value, save: () => undefined };
}

function mount(
  slot: AdSlotId,
  {
    provider = fakeProvider(),
    enabled = true,
    state = {},
    locale = 'en',
  }: { provider?: AdProvider; enabled?: boolean; state?: Partial<LauncherState>; locale?: Locale } = {},
) {
  const controller = createFakeController(state);
  const view = render(
    <I18nProvider locale={locale}>
      <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
        {/* The sponsored-content preference (#105) decides whether the slot may render. */}
        <AdPreferenceProvider store={preferenceStore(enabled ? 'on' : 'off')}>
          <AdsContext.Provider value={{ provider }}>
            <AdSlot slot={slot} />
          </AdsContext.Provider>
        </AdPreferenceProvider>
      </LauncherContext.Provider>
    </I18nProvider>,
  );
  return { provider, view };
}

const sponsored = () => screen.findByRole('region', { name: 'Sponsored' });

afterEach(cleanup);

describe('AdSlot', () => {
  it('renders nothing without a provider in context', () => {
    const { container } = render(<AdSlot slot="discover" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing and requests nothing without a preference store', async () => {
    const provider = fakeProvider();
    const controller = createFakeController();
    const { container } = render(
      <I18nProvider locale="en">
        <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
          <AdsContext.Provider value={{ provider }}>
            <AdSlot slot="discover" />
          </AdsContext.Provider>
        </LauncherContext.Provider>
      </I18nProvider>,
    );
    await act(() => Promise.resolve());
    expect(container).toBeEmptyDOMElement();
    expect(provider.get).not.toHaveBeenCalled();
  });

  it.each<AdSlotId>(['discover', 'sidebar'])(
    'shows the %s card with image, title, body, labels and action',
    async (slot) => {
      mount(slot);
      const card = await sponsored();
      expect(card).toHaveClass('ad-slot', `ad-slot-${slot}`);
      const image = card.querySelector('img') as HTMLImageElement;
      expect(image).toHaveAttribute('src', creative.imageDataUri);
      expect(image).toHaveAttribute('alt', '');
      expect(within(card).getByText('Sponsored')).toBeVisible();
      expect(within(card).getByText('Acme Racing')).toBeInTheDocument();
      expect(within(card).getByText('Track day tyres')).toBeInTheDocument();
      expect(within(card).getByText('Grip for every lap.')).toBeInTheDocument();
      const action = within(card).getByRole('button', { name: 'Learn more' });
      expect(action).toHaveAccessibleDescription('Track day tyres Opens in your browser');
      // The accent is reserved for Play.
      expect(action).not.toHaveClass('btn-primary');
      expect(within(card).getAllByRole('button')).toHaveLength(1);
    },
  );

  it('shows the About slot as a quiet setting row without an image', async () => {
    mount('about');
    const row = await sponsored();
    expect(row).toHaveClass('setting-row', 'ad-slot-about');
    expect(row.querySelector('img')).toBeNull();
    expect(within(row).getByRole('button', { name: 'Learn more' })).toHaveClass('btn', 'btn-sm');
  });

  it('hides a sponsor label that only repeats "Sponsored"', async () => {
    mount('discover', { provider: fakeProvider(() => Promise.resolve({ ...creative, sponsorLabel: 'sponsored' })) });
    const card = await sponsored();
    expect(within(card).getAllByText(/sponsored/i)).toHaveLength(1);
  });

  it('omits an empty body', async () => {
    mount('sidebar', { provider: fakeProvider(() => Promise.resolve({ ...creative, body: '', sponsorLabel: '' })) });
    const card = await sponsored();
    expect(card.querySelector('.ad-text')).toBeNull();
  });

  it('translates the label and the action in pt-BR', async () => {
    mount('discover', { locale: 'pt-BR' });
    const card = await screen.findByRole('region', { name: 'Patrocinado' });
    expect(within(card).getByRole('button', { name: 'Saiba mais' })).toHaveAccessibleDescription(
      'Track day tyres Abre no navegador',
    );
  });

  it('renders nothing and requests nothing when disabled', async () => {
    const { provider, view } = mount('discover', { enabled: false });
    await act(() => Promise.resolve());
    expect(view.container).toBeEmptyDOMElement();
    expect(provider.get).not.toHaveBeenCalled();
  });

  it('renders nothing while the launcher is loading', async () => {
    const { provider, view } = mount('sidebar', { state: { status: 'loading' } });
    await act(() => Promise.resolve());
    expect(view.container).toBeEmptyDOMElement();
    expect(provider.get).not.toHaveBeenCalled();
  });

  it('renders nothing while the ad loads', () => {
    const { view } = mount('discover', { provider: fakeProvider(() => new Promise(() => undefined)) });
    expect(view.container).toBeEmptyDOMElement();
  });

  it('clicks and reports with the slot and the creative id', async () => {
    const user = userEvent.setup();
    const { provider } = mount('sidebar');
    const card = await sponsored();
    await waitFor(() => expect(provider.reportShown).toHaveBeenCalledWith('sidebar', 'c1'));
    await user.click(within(card).getByRole('button', { name: 'Learn more' }));
    expect(provider.click).toHaveBeenCalledWith('sidebar', 'c1');
  });
});

describe('AdSlot styles', () => {
  const css = readFileSync(`${import.meta.dirname}/../styles.css`, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const adRules = [...css.matchAll(/([^{}]*\.ad-[^{}]*)\{([^}]*)\}/g)].map(([, selector = '', body = '']) => ({
    selector: selector.trim(),
    body,
  }));

  it('defines the ad rules', () => {
    expect(adRules.length).toBeGreaterThan(0);
  });

  it('uses no accent, no raw color and no raw font size', () => {
    for (const { selector, body } of adRules) {
      expect(body, selector).not.toMatch(/--accent/);
      expect(body, selector).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgba?\(/);
      for (const [, size = ''] of body.matchAll(/font-size\s*:\s*([^;]+)/g)) {
        expect(size.trim(), selector).toMatch(/^var\(--type-/);
      }
    }
  });

  it('never makes the sidebar card wider than the sidebar', () => {
    const sidebar = adRules.find(({ selector }) => selector === '.ad-slot-sidebar');
    expect(sidebar?.body).toMatch(/max-width:\s*100%/);
  });
});
