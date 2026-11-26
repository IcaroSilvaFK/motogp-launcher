// Binds tests/features/frontend/ad-preference.feature: the real App over a fake controller and an
// in-memory preference store. AdSlot belongs to another issue, so "a slot renders" is observed
// through a probe that reads the same hook an AdSlot placement will use.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import type { AdPreferenceStore } from '../../application';
import { App } from '../App';
import { AdPreferenceProvider, useAdsEnabled } from '../AdPreferenceContext';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

interface MemoryStore extends AdPreferenceStore {
  value: 'on' | 'off' | null;
}

function memoryStore(initial: 'on' | 'off' | null, throws = false): MemoryStore {
  const store: MemoryStore = {
    value: initial,
    load: () => {
      if (throws) throw new Error('storage unavailable');
      return store.value;
    },
    save: (value) => {
      if (throws) throw new Error('storage unavailable');
      store.value = value;
    },
  };
  return store;
}

function Probe() {
  return useAdsEnabled() ? <div data-testid="ad-slot" /> : null;
}

/** Mounts a probe that loads the preference from the same store, as a new placement would. */
function renderProbe(store: AdPreferenceStore) {
  cleanup();
  return render(
    <AdPreferenceProvider store={store}>
      <Probe />
    </AdPreferenceProvider>,
  );
}

function launch(store: AdPreferenceStore) {
  render(<App controller={createFakeController()} locale="en" adPreferenceStore={store} />);
}

const bar = () => screen.queryByRole('region', { name: 'Sponsored content' });

const feature = await loadFrontendFeature('ad-preference');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
  });

  Scenario('First launch shows a non-modal prompt', ({ Given, When, Then, And }) => {
    let store: MemoryStore;
    Given('the user never chose an ad preference', () => {
      store = memoryStore(null);
    });
    When('the launcher opens', () => {
      launch(store);
    });
    Then('a dismissible bar offers "Keep" and "Turn off ads"', () => {
      expect(bar()).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Keep' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Turn off ads' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Dismiss' })).toBeEnabled();
    });
    And('no dialog blocks the screen', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
      expect(screen.getByRole('main')).not.toHaveAttribute('inert');
    });
  });

  Scenario('Keeping ads hides the prompt for good', ({ Given, When, Then, And }) => {
    let store: MemoryStore;
    Given('the user never chose an ad preference', () => {
      store = memoryStore(null);
    });
    When('the launcher opens', () => {
      launch(store);
    });
    And('the user clicks "Keep"', async () => {
      await userEvent.setup().click(screen.getByRole('button', { name: 'Keep' }));
    });
    Then('the bar disappears', () => {
      expect(bar()).not.toBeInTheDocument();
    });
    And('the stored ad preference is "on"', () => {
      expect(store.value).toBe('on');
    });
    And('ads are enabled', () => {
      renderProbe(store);
      expect(screen.getByTestId('ad-slot')).toBeInTheDocument();
    });
  });

  Scenario('Turning ads off from the prompt', ({ Given, When, Then, And }) => {
    let store: MemoryStore;
    Given('the user never chose an ad preference', () => {
      store = memoryStore(null);
    });
    When('the launcher opens', () => {
      launch(store);
    });
    And('the user clicks "Turn off ads"', async () => {
      await userEvent.setup().click(screen.getByRole('button', { name: 'Turn off ads' }));
    });
    Then('the bar disappears', () => {
      expect(bar()).not.toBeInTheDocument();
    });
    And('the stored ad preference is "off"', () => {
      expect(store.value).toBe('off');
    });
    And('ads are disabled', () => {
      renderProbe(store);
      expect(screen.queryByTestId('ad-slot')).not.toBeInTheDocument();
    });
  });

  Scenario('A stored choice shows no prompt', ({ Given, When, Then }) => {
    let store: MemoryStore;
    Given('the stored ad preference is "on"', () => {
      store = memoryStore('on');
    });
    When('the launcher opens', () => {
      launch(store);
    });
    Then('no ad preference bar is shown', () => {
      expect(bar()).not.toBeInTheDocument();
    });
  });

  Scenario('Turning ads off removes every slot', ({ Given, When, Then, And }) => {
    let store: MemoryStore;
    Given('the stored ad preference is "on"', () => {
      store = memoryStore('on');
    });
    When('the user selects "Off" in Settings > Preferences', async () => {
      const user = userEvent.setup();
      launch(store);
      await user.click(screen.getByRole('button', { name: 'Settings' }));
      await user.selectOptions(screen.getByLabelText('Sponsored content'), 'off');
    });
    Then('no slot renders and no ad request is made', () => {
      renderProbe(store);
      expect(screen.queryByTestId('ad-slot')).not.toBeInTheDocument();
    });
    And('the stored ad preference is "off"', () => {
      expect(store.value).toBe('off');
    });
  });

  Scenario('The store fails', ({ Given, When, Then, And }) => {
    let store: MemoryStore;
    Given('the preference store throws', () => {
      store = memoryStore(null, true);
    });
    When('the launcher opens', () => {
      launch(store);
    });
    Then('ads are off', () => {
      renderProbe(store);
      expect(screen.queryByTestId('ad-slot')).not.toBeInTheDocument();
    });
    And('no ad preference bar is shown', () => {
      cleanup();
      launch(store);
      expect(bar()).not.toBeInTheDocument();
    });
  });
});
