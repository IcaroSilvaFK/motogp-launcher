// Minimal JSON Schema validator for catalog/catalog.schema.json. It covers only the keywords that
// schema uses: type, required, additionalProperties, properties, pattern, minLength, maxLength,
// minimum, const and $ref to "#/$defs/...". An unsupported keyword in the schema throws, so the
// schema cannot silently grow past what this checker enforces.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const schema = JSON.parse(readFileSync(join(here, '..', 'catalog', 'catalog.schema.json'), 'utf8'));

const ANNOTATIONS = new Set(['$schema', '$id', 'title', 'description', '$defs']);
const KEYWORDS = new Set([
  'type',
  'required',
  'additionalProperties',
  'properties',
  'pattern',
  'minLength',
  'maxLength',
  'minimum',
  'const',
  '$ref',
  'items',
]);

const isObject = (v) => typeof v === 'object' && v !== null && !Array.isArray(v);

function typeMatches(type, value) {
  switch (type) {
    case 'object':
      return isObject(value);
    case 'array':
      return Array.isArray(value);
    case 'string':
      return typeof value === 'string';
    case 'integer':
      return Number.isInteger(value);
    case 'number':
      return typeof value === 'number';
    case 'boolean':
      return typeof value === 'boolean';
    default:
      throw new Error(`Unsupported schema type: ${type}`);
  }
}

function resolveRef(ref) {
  const match = /^#\/\$defs\/([A-Za-z0-9_-]+)$/.exec(ref);
  const def = match ? schema.$defs?.[match[1]] : undefined;
  if (!def) throw new Error(`Unsupported or unknown $ref: ${ref}`);
  return def;
}

/** Appends to `problems` every violation of `node` by `value`; `path` names the value in messages. */
function check(node, value, path, problems) {
  for (const key of Object.keys(node)) {
    if (!KEYWORDS.has(key) && !ANNOTATIONS.has(key)) throw new Error(`Unsupported schema keyword: ${key}`);
  }
  if (node.$ref !== undefined) {
    check(resolveRef(node.$ref), value, path, problems);
    return;
  }
  if (node.const !== undefined && value !== node.const) {
    problems.push(`${path}: must be ${JSON.stringify(node.const)}`);
    return;
  }
  if (node.type !== undefined && !typeMatches(node.type, value)) {
    problems.push(`${path}: must be of type ${node.type}`);
    return;
  }
  if (typeof value === 'string') {
    if (node.minLength !== undefined && value.length < node.minLength) {
      problems.push(`${path}: must have at least ${node.minLength} character(s)`);
    }
    if (node.maxLength !== undefined && value.length > node.maxLength) {
      problems.push(`${path}: must have at most ${node.maxLength} characters`);
    }
    if (node.pattern !== undefined && !new RegExp(node.pattern, 'u').test(value)) {
      problems.push(`${path}: must match ${node.pattern}`);
    }
  }
  if (typeof value === 'number' && node.minimum !== undefined && value < node.minimum) {
    problems.push(`${path}: must be at least ${node.minimum}`);
  }
  if (Array.isArray(value) && node.items) {
    value.forEach((item, i) => check(node.items, item, `${path}[${i}]`, problems));
  }
  if (isObject(value)) {
    for (const name of node.required ?? []) {
      if (value[name] === undefined) problems.push(`${path}: missing required property "${name}"`);
    }
    const properties = node.properties ?? {};
    for (const [name, child] of Object.entries(value)) {
      if (child === undefined) continue;
      if (properties[name]) check(properties[name], child, `${path}.${name}`, problems);
      else if (node.additionalProperties === false) problems.push(`${path}: unexpected property "${name}"`);
    }
  }
}

/** Validates one catalog entry against `$defs/entry`; returns a list of problems (empty when valid). */
export function validateEntry(entry) {
  const problems = [];
  check(resolveRef('#/$defs/entry'), entry, 'entry', problems);
  return problems;
}

/** Key order of an entry, as declared by the schema. */
export const ENTRY_KEYS = Object.keys(resolveRef('#/$defs/entry').properties);
