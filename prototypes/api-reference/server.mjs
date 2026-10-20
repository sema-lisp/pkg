#!/usr/bin/env node
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { cssPath, loadData, markdown, normalHtml, printHtml } from './render.mjs';

const data = await loadData();
const css = await readFile(cssPath, 'utf8');
const release = `/packages/${data.package.name}/${data.package.version}`;
const base = `${release}/api`;
const source = await readFile(new URL('./fixture-release/source/package.sema', import.meta.url), 'utf8');
const escape = (value) => value.replace(/[&<>"']/g, (character) => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
}[character]));
const sourceHtml = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${data.package.name} ${data.package.version} source · Sema pkg</title><style>body{margin:2rem;background:#0c0c0c;color:#ddd4c4;font:14px/1.55 ui-monospace,SFMono-Regular,Menlo,monospace}a{color:#d5b86a}ol{margin:1.5rem 0;padding:0;list-style:none;counter-reset:line}li{display:grid;grid-template-columns:4rem minmax(0,1fr);gap:1rem;white-space:pre-wrap;scroll-margin-block:1rem}li::before{content:counter(line);counter-increment:line;color:#918878;text-align:right}li:target{background:#282012;outline:1px solid #d5b86a}code{overflow-wrap:anywhere}</style></head><body><a href="${base}">← ${data.package.name} API</a><h1>package.sema</h1><ol>${source.split('\n').map((line, index) => `<li id="L${index + 1}"><code>${escape(line)}</code></li>`).join('')}</ol></body></html>`;
const routes = new Map([
  [base, ['text/html; charset=utf-8', normalHtml(data)]],
  [`${base}/print`, ['text/html; charset=utf-8', printHtml(data, css)]],
  [`${base}.md`, ['text/markdown; charset=utf-8', markdown(data)]],
  [`${release}/source/package.sema`, ['text/html; charset=utf-8', sourceHtml]],
  ['/assets/site.css', ['text/css; charset=utf-8', css]],
]);
const port = Number(process.env.PORT ?? 4176);
createServer((request, response) => {
  const path = new URL(request.url, `http://${request.headers.host}`).pathname;
  const route = routes.get(path);
  if (!route) {
    response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    response.end(`Try ${base}`);
    return;
  }
  response.writeHead(200, { 'content-type': route[0] });
  response.end(route[1]);
}).listen(port, '127.0.0.1', () => console.log(`API reference prototype: http://127.0.0.1:${port}${base}`));
