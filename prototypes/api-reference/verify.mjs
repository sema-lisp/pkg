#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { cssPath, loadData, markdown, normalHtml, printHtml } from './render.mjs';

const data = await loadData();
const css = await readFile(cssPath, 'utf8');
const expectedNames = data.api.map((entry) => entry.name);
const views = {
  html: normalHtml(data, css),
  print: printHtml(data, css),
  markdown: markdown(data),
};
for (const [name, view] of Object.entries(views)) {
  const missing = expectedNames.filter((symbol) => !view.includes(symbol));
  if (missing.length) throw new Error(`${name} is missing: ${missing.join(', ')}`);
}
if (data.api.some((entry) => !entry.signature || !entry.docstring || !entry.source)) {
  throw new Error('the sema-retry fixture should have a signature, docstring, and source span for every export');
}
if (/<(?:link|script|img|iframe|video|audio|source|object|embed)\b/i.test(views.print)) {
  throw new Error('print page contains a resource-bearing element');
}
if (!views.print.includes('<style>')) throw new Error('print page does not inline its stylesheet');
if (!views.print.includes('break-inside:avoid')) throw new Error('print page does not protect API entries from splitting');
console.log(`verified ${expectedNames.length} exports across HTML, print HTML, and Markdown`);
console.log('verified self-contained print styling and source provenance fields');
