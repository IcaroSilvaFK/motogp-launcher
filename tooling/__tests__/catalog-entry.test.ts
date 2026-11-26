import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { run } from '../../scripts/catalog-entry.mjs';
import { validateEntry } from '../../scripts/catalog-schema-check.mjs';
import { MOD_BYTES, MOD_SHA256, MOD_URL, REQUIRED_FLAGS, createFakeDeps } from './catalogEntryFixture';

const without = (flag: string) => {
  const i = REQUIRED_FLAGS.indexOf(flag);
  return [...REQUIRED_FLAGS.slice(0, i), ...REQUIRED_FLAGS.slice(i + 2)];
};

describe('catalog-entry script', () => {
  it('prints the entry with keys in schema order and 2-space indentation', async () => {
    const f = createFakeDeps();
    expect(await run(REQUIRED_FLAGS, f.deps)).toBe(0);
    const text = f.out.join('');
    expect(Object.keys(JSON.parse(text))).toEqual([
      'id',
      'title',
      'author',
      'game',
      'category',
      'description',
      'version',
      'updatedAt',
      'pageUrl',
      'download',
    ]);
    expect(text).toContain('\n  "id": "my-livery"');
  });

  it('defaults updatedAt to today (UTC) from the injected clock', async () => {
    const f = createFakeDeps();
    await run(REQUIRED_FLAGS, f.deps);
    expect(JSON.parse(f.out.join('')).updatedAt).toBe('2026-11-26');
  });

  it('honours --updated-at and --thumbnail-url', async () => {
    const f = createFakeDeps();
    const argv = [...REQUIRED_FLAGS, '--updated-at', '2026-01-02', '--thumbnail-url', 'https://example.com/t.png'];
    expect(await run(argv, f.deps)).toBe(0);
    const entry = JSON.parse(f.out.join(''));
    expect(entry.updatedAt).toBe('2026-01-02');
    expect(entry.thumbnailUrl).toBe('https://example.com/t.png');
  });

  it('hashes a real file from disk with the default file reader', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'catalog-entry-'));
    try {
      const file = join(dir, 'mod.zip');
      writeFileSync(file, MOD_BYTES);
      const f = createFakeDeps();
      const { openFile: _unused, ...rest } = f.deps;
      void _unused;
      const argv = ['--file', file, ...REQUIRED_FLAGS.slice(2)];
      expect(await run(argv, rest)).toBe(0);
      expect(JSON.parse(f.out.join('')).download).toEqual({ url: MOD_URL, size: MOD_BYTES.length, sha256: MOD_SHA256 });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('verify succeeds when the hosted bytes match, following redirects', async () => {
    const f = createFakeDeps();
    expect(await run([...REQUIRED_FLAGS, '--verify'], f.deps)).toBe(0);
    expect(f.fetched).toHaveLength(1);
    expect(f.fetched[0]?.url).toBe(MOD_URL);
    expect(f.fetched[0]?.init?.redirect).toBe('follow');
    expect(JSON.parse(f.out.join('')).download.sha256).toBe(MOD_SHA256);
  });

  it('verify reports a size mismatch with expected and actual', async () => {
    const f = createFakeDeps({ fetchBody: Buffer.from('short') });
    expect(await run([...REQUIRED_FLAGS, '--verify'], f.deps)).toBe(1);
    expect(f.out).toEqual([]);
    const err = f.err.join('');
    expect(err).toMatch(/size mismatch/i);
    expect(err).toContain(String(MOD_BYTES.length));
    expect(err).toContain('5');
  });

  it('verify fails on an HTTP error status, reporting it', async () => {
    const f = createFakeDeps({ fetchStatus: 404 });
    expect(await run([...REQUIRED_FLAGS, '--verify'], f.deps)).toBe(1);
    expect(f.out).toEqual([]);
    expect(f.err.join('')).toContain('404');
  });

  it('verify fails when the fetch itself throws', async () => {
    const f = createFakeDeps();
    f.deps.fetch = async () => {
      throw new Error('network down');
    };
    expect(await run([...REQUIRED_FLAGS, '--verify'], f.deps)).toBe(1);
    expect(f.err.join('')).toContain('network down');
    expect(f.out).toEqual([]);
  });

  it('does not fetch without --verify', async () => {
    const f = createFakeDeps();
    await run(REQUIRED_FLAGS, f.deps);
    expect(f.fetched).toEqual([]);
  });

  it.each([
    '--file',
    '--url',
    '--id',
    '--title',
    '--author',
    '--game',
    '--category',
    '--description',
    '--version',
    '--page-url',
  ])('fails when %s is missing', async (flag) => {
    const f = createFakeDeps();
    expect(await run(without(flag), f.deps)).toBe(1);
    expect(f.out).toEqual([]);
    expect(f.err.join('')).toContain(flag);
  });

  it('fails on an unknown flag', async () => {
    const f = createFakeDeps();
    expect(await run([...REQUIRED_FLAGS, '--bogus', 'x'], f.deps)).toBe(1);
    expect(f.err.join('')).toContain('--bogus');
  });

  it('refuses a non-HTTPS --page-url and --thumbnail-url before opening the file', async () => {
    for (const extra of [
      ['--thumbnail-url', 'http://example.com/t.png'],
      ['--page-url', 'http://example.com/p'],
    ]) {
      const f = createFakeDeps();
      const argv = extra[0] === '--page-url' ? [...without('--page-url'), ...extra] : [...REQUIRED_FLAGS, ...extra];
      expect(await run(argv, f.deps)).toBe(1);
      expect(f.opened).toEqual([]);
      expect(f.out).toEqual([]);
    }
  });

  it('rejects a bad id slug through the schema check, listing the problem', async () => {
    const f = createFakeDeps();
    const argv = REQUIRED_FLAGS.map((a) => (a === 'my-livery' ? 'Bad Slug' : a));
    expect(await run(argv, f.deps)).toBe(1);
    expect(f.out).toEqual([]);
    expect(f.err.join('')).toMatch(/id/);
  });

  it('rejects a malformed --updated-at', async () => {
    const f = createFakeDeps();
    expect(await run([...REQUIRED_FLAGS, '--updated-at', '26/11/2026'], f.deps)).toBe(1);
    expect(f.err.join('')).toMatch(/updatedAt/);
  });

  it('rejects an empty file (size must be at least 1)', async () => {
    const f = createFakeDeps({ local: Buffer.alloc(0) });
    expect(await run(REQUIRED_FLAGS, f.deps)).toBe(1);
    expect(f.err.join('')).toMatch(/size/);
  });

  it('reports an unreadable file', async () => {
    const f = createFakeDeps();
    f.deps.openFile = async () => {
      throw new Error('ENOENT: no such file');
    };
    expect(await run(REQUIRED_FLAGS, f.deps)).toBe(1);
    expect(f.err.join('')).toContain('ENOENT');
  });
});

describe('catalog schema check', () => {
  const good = {
    id: 'my-livery',
    title: 'T',
    author: 'A',
    game: 'G',
    category: 'c',
    description: 'd',
    version: '1',
    updatedAt: '2026-11-26',
    pageUrl: 'https://example.com/p',
    download: { url: MOD_URL, size: 10, sha256: MOD_SHA256 },
  };

  it('accepts a valid entry', () => {
    expect(validateEntry(good)).toEqual([]);
  });

  it('accepts an entry without download', () => {
    const { download: _d, ...rest } = good;
    void _d;
    expect(validateEntry(rest)).toEqual([]);
  });

  it.each([
    ['missing required', { ...good, title: undefined }, /title/],
    ['extra property', { ...good, extra: 1 }, /extra/],
    ['bad slug pattern', { ...good, id: 'Bad_Slug' }, /id/],
    ['too long id', { ...good, id: 'a'.repeat(65) }, /id/],
    ['blank text', { ...good, title: '   ' }, /title/],
    ['empty text', { ...good, title: '' }, /title/],
    ['http url', { ...good, pageUrl: 'http://example.com' }, /pageUrl/],
    ['uppercase sha', { ...good, download: { ...good.download, sha256: MOD_SHA256.toUpperCase() } }, /sha256/],
    ['zero size', { ...good, download: { ...good.download, size: 0 } }, /size/],
    ['fractional size', { ...good, download: { ...good.download, size: 1.5 } }, /size/],
    ['string size', { ...good, download: { ...good.download, size: '10' } }, /size/],
    ['partial download', { ...good, download: { url: MOD_URL } }, /download/],
    ['non-object', 'nope', /object/],
  ])('rejects: %s', (_name, entry, expected) => {
    const problems = validateEntry(entry);
    expect(problems.length).toBeGreaterThan(0);
    expect(problems.join('\n')).toMatch(expected);
  });
});
