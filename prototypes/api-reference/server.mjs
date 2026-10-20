#!/usr/bin/env node
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { cssPath, loadData, markdown, normalHtml, printHtml } from './render.mjs';

const data = await loadData();
const css = await readFile(cssPath, 'utf8');
const release = `/packages/${data.package.name}/${data.package.version}`;
const base = `${release}/api`;
const source = await readFile(new URL('./fixture-release/source/package.sema', import.meta.url), 'utf8');
const routes = new Map([
  [base, ['text/html; charset=utf-8', normalHtml(data)]],
  [`${base}/print`, ['text/html; charset=utf-8', printHtml(data, css)]],
  [`${base}.md`, ['text/markdown; charset=utf-8', markdown(data)]],
  [`${release}/source/package.sema`, ['text/plain; charset=utf-8', source]],
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
}).listen(port, () => console.log(`API reference prototype: http://127.0.0.1:${port}${base}`));
