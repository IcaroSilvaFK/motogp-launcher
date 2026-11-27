import { describe, expect, it } from 'vitest';
import { FakeDiagnostics } from '../../__tests__/fakeDiagnostics';
import { buildDiagnosticsReport, copyDiagnostics } from '../copyDiagnostics';

const facts = { version: '0.1.0', gameDir: 'D:/Steam/MotoGP25', modCount: 3, activeProfile: 'Mine' };

describe('buildDiagnosticsReport', () => {
  it('lists the facts and the log lines', () => {
    const text = buildDiagnosticsReport(facts, { os: 'Windows 11', logLines: ['a', 'b'] });
    expect(text).toContain('Launcher version: 0.1.0');
    expect(text).toContain('OS: Windows 11');
    expect(text).toContain('Game folder: D:/Steam/MotoGP25');
    expect(text).toContain('Installed mods: 3');
    expect(text).toContain('Active profile: Mine');
    expect(text.endsWith('Recent log (2 lines):\na\nb')).toBe(true);
  });

  it('says so when facts are missing', () => {
    const text = buildDiagnosticsReport(
      { version: null, gameDir: null, modCount: 0, activeProfile: null },
      { os: 'Windows 11', logLines: [] },
    );
    expect(text).toContain('Launcher version: unknown');
    expect(text).toContain('Game folder: not detected');
    expect(text).toContain('Active profile: none');
    expect(text).toContain('(empty)');
  });
});

describe('copyDiagnostics', () => {
  it('copies the report once', async () => {
    const diagnostics = new FakeDiagnostics();
    await copyDiagnostics(diagnostics, facts);
    expect(diagnostics.copied).toHaveLength(1);
    expect(diagnostics.copied[0]).toContain('Installed mods: 3');
  });

  it('rejects when the clipboard fails', async () => {
    const diagnostics = new FakeDiagnostics();
    diagnostics.failCopy = true;
    await expect(copyDiagnostics(diagnostics, facts)).rejects.toThrow('clipboard unavailable');
  });
});
