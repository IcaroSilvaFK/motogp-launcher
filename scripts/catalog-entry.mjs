// Builds a ready-to-paste catalog entry for a downloadable mod: computes the size and the
// lowercase SHA-256 of the local file (streamed, mod files can be GBs), optionally verifies that
// the hosted URL serves the very same bytes, validates the entry against catalog.schema.json and
// prints it as JSON.
//
// Usage:
//   node scripts/catalog-entry.mjs --file <path> --url <https-url> --id <slug> --title <t> \
//     --author <a> --game <g> --category <c> --description <d> --version <v> --page-url <https-url> \
//     [--thumbnail-url <https-url>] [--updated-at YYYY-MM-DD] [--verify]
// Also: corepack pnpm catalog-entry -- <same flags>
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { ENTRY_KEYS, validateEntry } from './catalog-schema-check.mjs';

const VALUE_FLAGS = {
  '--file': 'file',
  '--url': 'url',
  '--id': 'id',
  '--title': 'title',
  '--author': 'author',
  '--game': 'game',
  '--category': 'category',
  '--description': 'description',
  '--version': 'version',
  '--page-url': 'pageUrl',
  '--thumbnail-url': 'thumbnailUrl',
  '--updated-at': 'updatedAt',
};
const REQUIRED = [
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
];

class UsageError extends Error {}

function parseArgs(argv) {
  const values = {};
  let verify = false;
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === '--') {
      // corepack pnpm forwards the separator after `catalog-entry --`.
    } else if (flag === '--verify') {
      verify = true;
    } else if (VALUE_FLAGS[flag]) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) throw new UsageError(`Missing value for ${flag}`);
      values[VALUE_FLAGS[flag]] = value;
      i += 1;
    } else {
      throw new UsageError(`Unknown argument: ${flag}`);
    }
  }
  for (const flag of REQUIRED) {
    if (values[VALUE_FLAGS[flag]] === undefined) throw new UsageError(`Missing required flag ${flag}`);
  }
  return { values, verify };
}

const isHttps = (value) => /^https:\/\//.test(value);

/** Hashes an async iterable of chunks, counting bytes. */
async function hashStream(chunks) {
  const hash = createHash('sha256');
  let size = 0;
  for await (const chunk of chunks) {
    hash.update(chunk);
    size += chunk.length;
  }
  return { size, sha256: hash.digest('hex') };
}

const defaultDeps = {
  openFile: async (path) => {
    const { size } = await stat(path);
    return { size, stream: createReadStream(path) };
  },
  fetch: (url, init) => globalThis.fetch(url, init),
  stdout: process.stdout,
  stderr: process.stderr,
  now: () => new Date(),
};

/**
 * @typedef {object} Deps
 * @property {(path: string) => Promise<{ size: number, stream: AsyncIterable<Uint8Array> }>} [openFile]
 * @property {(url: string, init?: RequestInit) => Promise<{ ok: boolean, status: number, body: ReadableStream<Uint8Array> | null }>} [fetch]
 * @property {{ write: (text: string) => unknown }} [stdout]
 * @property {{ write: (text: string) => unknown }} [stderr]
 * @property {() => Date} [now]
 */

/**
 * Runs the CLI. Returns the exit code; writes the entry to stdout only on success.
 * @param {string[]} argv
 * @param {Deps} [deps] injectable file reader, fetch, output streams and clock
 */
export async function run(argv, deps = {}) {
  const { openFile, fetch, stdout, stderr, now } = { ...defaultDeps, ...deps };
  const fail = (message) => {
    stderr.write(`${message}\n`);
    return 1;
  };
  try {
    const { values, verify } = parseArgs(argv);
    for (const [flag, key] of [
      ['--url', 'url'],
      ['--page-url', 'pageUrl'],
      ['--thumbnail-url', 'thumbnailUrl'],
    ]) {
      if (values[key] !== undefined && !isHttps(values[key])) {
        return fail(`${flag} must be an https:// URL (got ${values[key]})`);
      }
    }

    const file = await openFile(values.file);
    const local = await hashStream(file.stream);

    if (verify) {
      const response = await fetch(values.url, { redirect: 'follow' });
      if (!response.ok) return fail(`Verification failed: ${values.url} answered HTTP ${response.status}`);
      if (!response.body) return fail(`Verification failed: ${values.url} returned no body`);
      const remote = await hashStream(
        /** @type {AsyncIterable<Uint8Array>} */ (/** @type {unknown} */ (response.body)),
      );
      const problems = [];
      if (remote.size !== local.size) {
        problems.push(`Size mismatch: local file has ${local.size} bytes, URL served ${remote.size}`);
      }
      if (remote.sha256 !== local.sha256) {
        problems.push(`SHA256 mismatch: local ${local.sha256}, URL served ${remote.sha256}`);
      }
      if (problems.length > 0) return fail(problems.join('\n'));
    }

    const entry = {
      id: values.id,
      title: values.title,
      author: values.author,
      game: values.game,
      category: values.category,
      description: values.description,
      version: values.version,
      updatedAt: values.updatedAt ?? now().toISOString().slice(0, 10),
      pageUrl: values.pageUrl,
      thumbnailUrl: values.thumbnailUrl,
      download: { url: values.url, size: local.size, sha256: local.sha256 },
    };
    const ordered = Object.fromEntries(ENTRY_KEYS.filter((k) => entry[k] !== undefined).map((k) => [k, entry[k]]));

    const problems = validateEntry(ordered);
    if (problems.length > 0) return fail(`The entry does not match catalog.schema.json:\n- ${problems.join('\n- ')}`);

    stdout.write(`${JSON.stringify(ordered, null, 2)}\n`);
    return 0;
  } catch (error) {
    return fail(error instanceof Error ? error.message : String(error));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await run(process.argv.slice(2));
}
