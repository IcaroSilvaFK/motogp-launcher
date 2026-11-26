import { describe, expect, it, vi } from 'vitest';
import type { UsagePreference } from '../../../domain';
import type { DownloadStats } from '../../ports/DownloadStats';
import { reportCatalogDownload } from '../reportCatalogDownload';

function stats(report: DownloadStats['report'] = () => Promise.resolve()) {
  return {
    report: vi.fn(report),
    totals: vi.fn(() => Promise.resolve(null)),
    daily: vi.fn(() => Promise.resolve(null)),
  } satisfies DownloadStats;
}

const preference = (value: UsagePreference) => ({ get: () => value });

describe('reportCatalogDownload', () => {
  it('reports the mod once when sharing is on', () => {
    const port = stats();
    reportCatalogDownload({ stats: port, preference: preference('on') }, 'rebalanced-ai');
    expect(port.report).toHaveBeenCalledTimes(1);
    expect(port.report).toHaveBeenCalledWith('rebalanced-ai');
  });

  it('sends nothing when the user opted out', () => {
    const port = stats();
    reportCatalogDownload({ stats: port, preference: preference('off') }, 'rebalanced-ai');
    expect(port.report).not.toHaveBeenCalled();
  });

  it('returns at once without waiting for the report', () => {
    const port = stats(() => new Promise<void>(() => undefined));
    expect(reportCatalogDownload({ stats: port, preference: preference('on') }, 'a')).toBeUndefined();
  });

  it('swallows a rejected report', async () => {
    const rejection = Promise.reject(new Error('offline'));
    const port = stats(() => rejection);
    reportCatalogDownload({ stats: port, preference: preference('on') }, 'a');
    // An unhandled rejection would fail the run; settle it first.
    await rejection.catch(() => undefined);
    await Promise.resolve();
    expect(port.report).toHaveBeenCalledTimes(1);
  });

  it('swallows a port that throws synchronously', () => {
    const port = stats(() => {
      throw new Error('not wired');
    });
    expect(() => reportCatalogDownload({ stats: port, preference: preference('on') }, 'a')).not.toThrow();
  });

  it('sends nothing when the preference cannot be read', () => {
    const port = stats();
    const broken = {
      get: (): UsagePreference => {
        throw new Error('broken');
      },
    };
    expect(() => reportCatalogDownload({ stats: port, preference: broken }, 'a')).not.toThrow();
    expect(port.report).not.toHaveBeenCalled();
  });
});
