#!/usr/bin/env node
/**
 * Generate the prototype's API document model from one released package tree.
 *
 * This deliberately supports only the source shapes documented in the proposal:
 * a literal `(module … (export name …) …)` and direct `(define (name args …)
 * "docstring" …)` forms.  The proposed registry implementation must use
 * `sema-reader`, rather than this small fixture generator, for full language
 * correctness and spans.
 */
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import process from 'node:process';

const here = new URL('.', import.meta.url).pathname;
const fixtureRoot = resolve(here, './fixture-release');
const defaultSource = resolve(fixtureRoot, 'source');
const sourceDir = resolve(process.argv[2] ?? defaultSource);
const output = resolve(process.argv[3] ?? `${here}/generated/api.json`);

function tokenise(source) {
  const tokens = [];
  let i = 0;
  let line = 1;
  const whitespace = /\s/;
  while (i < source.length) {
    const c = source[i];
    if (c === ';') {
      while (i < source.length && source[i] !== '\n') i += 1;
      continue;
    }
    if (whitespace.test(c)) {
      if (c === '\n') line += 1;
      i += 1;
      continue;
    }
    if (c === '(' || c === ')') {
      tokens.push({ kind: c, raw: c, line });
      i += 1;
      continue;
    }
    if (c === '"') {
      const start = line;
      let raw = '"';
      i += 1;
      while (i < source.length) {
        const ch = source[i];
        raw += ch;
        i += 1;
        if (ch === '\n') line += 1;
        if (ch === '\\' && i < source.length) {
          raw += source[i];
          if (source[i] === '\n') line += 1;
          i += 1;
          continue;
        }
        if (ch === '"') break;
      }
      if (!raw.endsWith('"')) throw new Error(`unterminated string at line ${start}`);
      // Sema permits literal newlines in strings, whereas JSON does not. This
      // fixture decoder only needs the string escapes used in source docstrings;
      // the production extractor uses sema-reader's decoded Value instead.
      const value = raw.slice(1, -1).replace(/\\(.)/gs, (_match, escaped) => ({
        n: '\n', r: '\r', t: '\t', '"': '"', '\\': '\\',
      }[escaped] ?? escaped));
      tokens.push({ kind: 'string', raw, value, line: start });
      continue;
    }
    const start = line;
    let raw = '';
    while (i < source.length && !whitespace.test(source[i]) && !'();'.includes(source[i])) {
      raw += source[i++];
    }
    if (!raw) throw new Error(`unsupported reader form at line ${line}`);
    tokens.push({ kind: 'symbol', raw, value: raw, line: start });
  }
  return tokens;
}

function parse(tokens) {
  let position = 0;
  function form() {
    const token = tokens[position++];
    if (!token) throw new Error('unexpected end of source');
    if (token.kind !== '(') {
      if (token.kind === ')') throw new Error(`unexpected ) at line ${token.line}`);
      return token;
    }
    const items = [];
    while (tokens[position]?.kind !== ')') {
      if (!tokens[position]) throw new Error(`unclosed list at line ${token.line}`);
      items.push(form());
    }
    const end = tokens[position++];
    return { kind: 'list', items, line: token.line, endLine: end.line };
  }
  const forms = [];
  while (position < tokens.length) forms.push(form());
  return forms;
}

function symbol(node) { return node?.kind === 'symbol' ? node.value : null; }
function render(node) {
  if (node.kind === 'list') return `(${node.items.map(render).join(' ')})`;
  return node.raw;
}

function parseToml(text) {
  return Object.fromEntries([...text.matchAll(/^(name|version|description|repository)\s*=\s*"([^"]*)"\s*$/gm)]
    .map(([, key, value]) => [key, value]));
}

function markdownSection(markdown, heading) {
  const lines = markdown.split('\n');
  const start = lines.findIndex((line) => line === `## ${heading}`);
  if (start < 0) return null;
  let end = start + 1;
  while (end < lines.length && !lines[end].startsWith('## ')) end += 1;
  return { markdown: lines.slice(start + 1, end).join('\n').trim(), startLine: start + 2, endLine: end };
}

const [source, manifest, readme, releaseMetadata, tarball] = await Promise.all([
  readFile(`${sourceDir}/package.sema`, 'utf8'),
  readFile(`${sourceDir}/sema.toml`, 'utf8'),
  readFile(`${sourceDir}/README.md`, 'utf8'),
  readFile(resolve(fixtureRoot, 'metadata.json'), 'utf8').then(JSON.parse),
  readFile(resolve(fixtureRoot, 'sema-retry-0.1.0.tar.gz')),
]);
const tarballSha256 = createHash('sha256').update(tarball).digest('hex');
if (tarballSha256 !== releaseMetadata.checksum_sha256) {
  throw new Error(`tarball SHA-256 does not match registry metadata: ${tarballSha256}`);
}
for (const [path, contents] of Object.entries({ 'package.sema': source, 'sema.toml': manifest, 'README.md': readme })) {
  const actual = createHash('sha256').update(contents).digest('hex');
  if (actual !== releaseMetadata.extracted_file_sha256?.[path]) {
    throw new Error(`${path} does not match the verified extracted release file`);
  }
}
const forms = parse(tokenise(source));
const moduleForm = forms.find((form) => form.kind === 'list' && symbol(form.items[0]) === 'module');
if (!moduleForm) throw new Error('package.sema does not have a literal module form');
const exportForm = moduleForm.items.find((form) => form.kind === 'list' && symbol(form.items[0]) === 'export');
if (!exportForm) throw new Error('module does not have a literal export form');
const exports = exportForm.items.slice(1).map(symbol);
if (exports.some((name) => !name)) throw new Error('export form contains a non-symbol export');

const definitions = new Map();
for (const form of moduleForm.items.slice(2)) {
  if (form.kind !== 'list' || symbol(form.items[0]) !== 'define') continue;
  const signature = form.items[1];
  if (signature?.kind !== 'list') continue;
  const name = symbol(signature.items[0]);
  if (!name) continue;
  const docstring = form.items[2]?.kind === 'string' ? form.items[2].value : null;
  definitions.set(name, { signature: render(signature), docstring, line: form.line });
}

const api = exports.map((name) => {
  const definition = definitions.get(name);
  return {
    name,
    kind: definition ? 'function' : 'unresolved-export',
    signature: definition?.signature ?? null,
    docstring: definition?.docstring ?? null,
    source: definition ? { path: 'package.sema', line: definition.line } : null,
  };
});
const pkg = parseToml(manifest);
const quickStart = markdownSection(readme, 'Quick start');
const data = {
  schemaVersion: 1,
  package: {
    name: pkg.name,
    version: pkg.version,
    description: pkg.description,
    repository: pkg.repository,
    entrypoint: 'package.sema',
  },
  provenance: {
    tarballSha256,
    tarballSizeBytes: tarball.byteLength,
    registryDownloadUrl: releaseMetadata.download_url,
    registryPublishedAt: releaseMetadata.published_at,
    sourceSha256: createHash('sha256').update(source).digest('hex'),
    manifestSha256: createHash('sha256').update(manifest).digest('hex'),
    readmeSha256: createHash('sha256').update(readme).digest('hex'),
    extraction: 'prototype fixture parser; production proposal requires sema-reader AST parsing',
  },
  api,
  readme: quickStart && { section: 'Quick start', path: 'README.md', ...quickStart },
};
await mkdir(dirname(output), { recursive: true });
await writeFile(output, `${JSON.stringify(data, null, 2)}\n`);
console.log(`wrote ${output}: ${api.length} exports, ${api.filter((item) => item.docstring).length} source docstrings`);
