import { describe, expect, it } from 'vitest';
import {
  MAX_SCREENSHOTS,
  PublishError,
  parseModVersion,
  parseMyMods,
  parsePublishProgress,
  suggestSlug,
  toPublishError,
  toPublishRequest,
  validatePublishForm,
} from '../publishing';
import type { PublishForm } from '../publishing';

function form(overrides: Partial<PublishForm> = {}): PublishForm {
  return {
    modId: 'brazil-livery',
    title: 'Brazil Livery',
    description: 'A livery',
    longDescription: '',
    category: 'livery',
    game: 'motogp25',
    version: '1.0',
    // Accented Windows path on purpose: real users have them.
    file: 'C:\\mods\\Usuário Padrão\\brazil.ZIP',
    screenshots: [],
    ownershipConfirmed: true,
    ...overrides,
  };
}

const rawVersion = {
  modId: 'a',
  version: '1.0',
  title: 'T',
  description: 'D',
  category: 'c',
  game: 'g',
  fileName: 'a.zip',
  size: 10,
  sha256: 'ab',
  status: 'pending',
  screenshotCount: 2,
  uploadedAt: '2026-01-01T00:00:00Z',
};

describe('Given the publish form rules', () => {
  it('When everything is valid Then there are no errors', () => {
    expect(validatePublishForm(form())).toEqual({});
  });

  it.each([
    ['', 'modIdRequired'],
    ['Brazil', 'modIdInvalid'],
    ['a--b', 'modIdInvalid'],
    ['-a', 'modIdInvalid'],
    ['a'.repeat(65), 'modIdTooLong'],
    ['mine', 'modIdReserved'],
    ['uploads', 'modIdReserved'],
    ['stats', 'modIdReserved'],
  ])('When the mod id is %j Then %s', (modId, key) => {
    expect(validatePublishForm(form({ modId })).modId).toBe(key);
  });

  it('When a mod id is 64 chars of a-z Then it is accepted', () => {
    expect(validatePublishForm(form({ modId: 'a'.repeat(64) })).modId).toBeUndefined();
  });

  it('When text fields are blank or too long Then they get stable keys', () => {
    const errors = validatePublishForm(
      form({ title: '  ', description: '', category: ' ', game: '', longDescription: 'x'.repeat(20001) }),
    );
    expect(errors).toMatchObject({
      title: 'titleRequired',
      description: 'descriptionRequired',
      category: 'categoryRequired',
      game: 'gameRequired',
      longDescription: 'longDescriptionTooLong',
    });
    expect(validatePublishForm(form({ title: 'x'.repeat(121) })).title).toBe('titleTooLong');
    expect(validatePublishForm(form({ description: 'x'.repeat(1001) })).description).toBe('descriptionTooLong');
    expect(validatePublishForm(form({ title: 'x'.repeat(120), description: 'x'.repeat(1000) }))).toEqual({});
  });

  it.each([
    ['', 'versionRequired'],
    ['1 0', 'versionInvalid'],
    ['1/0', 'versionInvalid'],
    ['1'.repeat(33), 'versionInvalid'],
  ])('When the version is %j Then %s', (version, key) => {
    expect(validatePublishForm(form({ version })).version).toBe(key);
  });

  it('When the version uses letters, digits and . _ + - Then it is accepted', () => {
    expect(validatePublishForm(form({ version: '1.0.0-beta_2+x' })).version).toBeUndefined();
  });

  it('When the file is missing or has another extension Then it is refused', () => {
    expect(validatePublishForm(form({ file: null })).file).toBe('fileRequired');
    expect(validatePublishForm(form({ file: 'a.exe' })).file).toBe('fileUnsupported');
    expect(validatePublishForm(form({ file: 'a.7Z' })).file).toBeUndefined();
    expect(validatePublishForm(form({ file: 'a.pak' })).file).toBeUndefined();
  });

  it('When screenshots are too many or not images Then they are refused', () => {
    const many = Array.from({ length: MAX_SCREENSHOTS + 1 }, (_, i) => `${i}.png`);
    expect(validatePublishForm(form({ screenshots: many })).screenshots).toBe('screenshotsTooMany');
    expect(validatePublishForm(form({ screenshots: ['a.gif'] })).screenshots).toBe('screenshotUnsupported');
    const ok = ['a.PNG', 'b.jpg', 'c.jpeg', 'd.webp'];
    expect(validatePublishForm(form({ screenshots: ok })).screenshots).toBeUndefined();
  });

  it('When ownership is not confirmed Then it is refused', () => {
    expect(validatePublishForm(form({ ownershipConfirmed: false })).ownershipConfirmed).toBe('ownershipRequired');
  });
});

describe('Given a title', () => {
  it('When suggesting a slug Then it is lower-case, hyphenated and ASCII', () => {
    expect(suggestSlug('Brazil Livery')).toBe('brazil-livery');
    expect(suggestSlug('  Pista Épica -- 2025!  ')).toBe('pista-epica-2025');
    expect(suggestSlug('???')).toBe('');
    expect(suggestSlug('x'.repeat(100))).toHaveLength(64);
    expect(suggestSlug('Mine')).toBe('');
  });
});

describe('Given a form', () => {
  it('When building the request Then text is trimmed and an empty long description is omitted', () => {
    const request = toPublishRequest(form({ title: ' T ', longDescription: '  ', screenshots: ['a.png'] }));
    expect(request).toEqual({
      modId: 'brazil-livery',
      title: 'T',
      description: 'A livery',
      category: 'livery',
      game: 'motogp25',
      version: '1.0',
      file: 'C:\\mods\\Usuário Padrão\\brazil.ZIP',
      screenshots: ['a.png'],
      ownershipConfirmed: true,
    });
    expect(toPublishRequest(form({ longDescription: ' more ' })).longDescription).toBe('more');
  });
});

describe('Given backend answers', () => {
  it('When parsing a version Then optional fields are kept only when present', () => {
    const raw = { ...rawVersion, status: 'rejected', rejectionReason: 'bad' };
    expect(parseModVersion(raw)).toMatchObject({ status: 'rejected', rejectionReason: 'bad', size: 10 });
    expect(parseModVersion(rawVersion)).not.toHaveProperty('rejectionReason');
    expect(parseModVersion({ ...raw, status: 'weird' })).toBeNull();
    expect(parseModVersion(null)).toBeNull();
    expect(parseModVersion({ ...raw, size: 'x' })).toBeNull();
  });

  it('When parsing my mods Then malformed entries are dropped', () => {
    const parsed = parseMyMods([{ id: 'a', versions: [rawVersion, 3] }, { id: '' }, 4]);
    expect(parsed).toHaveLength(1);
    expect(parsed?.[0]?.versions).toHaveLength(1);
    expect(parseMyMods('nope')).toBeNull();
  });

  it('When parsing progress Then malformed payloads are null', () => {
    expect(parsePublishProgress({ sent: 1, total: 2, phase: 'uploading' })).toEqual({
      sent: 1,
      total: 2,
      phase: 'uploading',
    });
    expect(parsePublishProgress({ sent: 1, total: 2, phase: 'x' })).toBeNull();
    expect(parsePublishProgress(null)).toBeNull();
  });

  it('When mapping errors Then known codes are kept and unknown ones become server_error', () => {
    expect(toPublishError({ code: 'file_too_large', message: 'big' })).toMatchObject({
      code: 'file_too_large',
      message: 'big',
    });
    expect(toPublishError({ code: 'weird', message: 'x' }).code).toBe('server_error');
    expect(toPublishError('boom').code).toBe('server_error');
    const own = new PublishError('offline', 'x');
    expect(toPublishError(own)).toBe(own);
  });
});
