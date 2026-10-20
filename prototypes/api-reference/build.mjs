#!/usr/bin/env node
// Materialize self-contained review artifacts from the checked-in API model.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { cssPath, loadData, markdown, normalHtml, printHtml } from './render.mjs';

const root = new URL('.', import.meta.url);
const output = new URL('./generated/', root);
const data = await loadData();
const css = await readFile(cssPath, 'utf8');
const prefix = `${data.package.name}-${data.package.version}-api`;
await mkdir(output, { recursive: true });
await Promise.all([
  writeFile(new URL(`./${prefix}.html`, output), normalHtml(data, css)),
  writeFile(new URL(`./${prefix}-print.html`, output), printHtml(data, css)),
  writeFile(new URL(`./${prefix}.md`, output), markdown(data)),
]);
console.log(`wrote standalone review artifacts in ${fileURLToPath(output)}`);
