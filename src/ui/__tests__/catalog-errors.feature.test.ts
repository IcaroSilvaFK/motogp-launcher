// Binds tests/features/frontend/catalog-errors.feature: the backend codes are known and translated.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { expect } from 'vitest';
import { ERROR_CODES, toCommandError } from '../../domain';
import { en } from '../i18n/en';
import { ptBR } from '../i18n/pt-BR';
import { loadFrontendFeature } from './features';

const feature = await loadFrontendFeature('catalog-errors');

describeFeature(feature, ({ Scenario }) => {
  Scenario('Catalog error codes are known and translated', ({ Then }) => {
    Then('"catalog_unavailable" and "catalog_unsupported_schema" are error codes with en and pt-BR messages', () => {
      for (const code of ['catalog_unavailable', 'catalog_unsupported_schema'] as const) {
        expect(ERROR_CODES).toContain(code);
        expect(en.errors[code]).toBeTruthy();
        expect(ptBR.errors[code]).toBeTruthy();
        // A backend failure keeps its code, so the UI can translate it.
        expect(toCommandError({ code, message: 'backend text' })).toEqual({ code, message: 'backend text' });
      }
    });
  });

  Scenario('Open page error codes are known and translated', ({ Then }) => {
    Then(
      '"catalog_entry_not_found", "catalog_page_not_allowed" and "open_page_failed" are error codes with en and pt-BR messages',
      () => {
        for (const code of ['catalog_entry_not_found', 'catalog_page_not_allowed', 'open_page_failed'] as const) {
          expect(ERROR_CODES).toContain(code);
          expect(en.errors[code]).toBeTruthy();
          expect(ptBR.errors[code]).toBeTruthy();
          expect(toCommandError({ code, message: 'backend text' })).toEqual({ code, message: 'backend text' });
        }
      },
    );
  });
});
