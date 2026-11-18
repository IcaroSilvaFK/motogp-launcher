// Frontend scenarios from tests/features/frontend/scaffold.feature (Vitest + Testing Library).
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from '../App';

describe('Given the launcher started in the WebView', () => {
  describe('When the interface is rendered', () => {
    it('renders_launcher_shell: Then I see the accessible MotoGP Launcher shell', () => {
      render(<App />);

      expect(screen.getByRole('banner')).toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 1, name: 'MotoGP Launcher' })).toBeInTheDocument();
      expect(screen.getByRole('main')).toBeInTheDocument();
    });
  });
});
