// Shared in-memory fakes for the catalog entry script tests: no real files, no network.
import { createHash } from 'node:crypto';

export const MOD_BYTES = Buffer.from('fake mod archive bytes');
export const MOD_SHA256 = createHash('sha256').update(MOD_BYTES).digest('hex');
export const MOD_URL = 'https://github.com/someone/mods/releases/download/v1/livery.zip';

export const REQUIRED_FLAGS = [
  '--file',
  'livery.zip',
  '--url',
  MOD_URL,
  '--id',
  'my-livery',
  '--title',
  'My Livery',
  '--author',
  'Someone',
  '--game',
  'MotoGP 26',
  '--category',
  'liveries',
  '--description',
  'A test livery.',
  '--version',
  '1.0',
  '--page-url',
  'https://example.com/my-livery',
];

export function createFakeDeps(options: { local?: Buffer; fetchBody?: Buffer; fetchStatus?: number } = {}) {
  const local = options.local ?? MOD_BYTES;
  const out: string[] = [];
  const err: string[] = [];
  const opened: string[] = [];
  const fetched: { url: string; init: RequestInit | undefined }[] = [];
  return {
    out,
    err,
    opened,
    fetched,
    deps: {
      openFile: async (path: string) => {
        opened.push(path);
        // Two chunks, to prove the hash is computed over a stream and not over one buffer.
        const half = Math.floor(local.length / 2);
        async function* chunks() {
          yield local.subarray(0, half);
          yield local.subarray(half);
        }
        return { size: local.length, stream: chunks() };
      },
      fetch: async (url: string, init?: RequestInit) => {
        fetched.push({ url, init });
        return new Response(options.fetchBody ?? MOD_BYTES, { status: options.fetchStatus ?? 200 });
      },
      stdout: { write: (text: string) => void out.push(text) },
      stderr: { write: (text: string) => void err.push(text) },
      now: () => new Date('2026-11-26T23:30:00Z'),
    },
  };
}
