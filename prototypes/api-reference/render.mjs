import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const here = new URL('.', import.meta.url);
const escape = (value = '') => String(value).replace(/[&<>'"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' }[char]));
const slug = (name) => name.replaceAll('/', '-').replace(/!/g, '-bang');
const releaseBase = (data) => `/packages/${encodeURIComponent(data.package.name)}/${encodeURIComponent(data.package.version)}`;
const apiBase = (data) => `${releaseBase(data)}/api`;
const sourceFile = (data, path, line) => `${releaseBase(data)}/source/${encodeURIComponent(path)}#L${line}`;

export async function loadData() {
  return JSON.parse(await readFile(new URL('./generated/api.json', here), 'utf8'));
}

function functionList(data) {
  return data.api.map((item) => `
    <li><a href="#${slug(item.name)}"><code>${escape(item.name)}</code></a></li>`).join('');
}

function sourceNotice(data) {
  return `<details class="provenance">
    <summary>Extraction details</summary>
    <p>Signatures, exported names, and descriptions below come from the released <code>${escape(data.package.entrypoint)}</code>. README prose is kept separate.</p>
    <dl>
      <div><dt>Registry tarball</dt><dd><code>${escape(data.provenance.tarballSha256)}</code></dd></div>
      <div><dt>Entrypoint digest</dt><dd><code>${data.provenance.sourceSha256.slice(0, 16)}…</code></dd></div>
      <div><dt>Extraction</dt><dd>${escape(data.provenance.extraction)}</dd></div>
    </dl>
  </details>`;
}

function apiEntries(data) {
  return data.api.map((item) => `
    <article class="api-entry" id="${slug(item.name)}">
      <div class="api-entry-title"><h2><code>${escape(item.name)}</code></h2>${item.source ? `<a class="source-link" href="${escape(sourceFile(data, item.source.path, item.source.line))}">${escape(item.source.path)}:${item.source.line}</a>` : '<span class="source-link">unresolved export</span>'}</div>
      ${item.signature ? `<pre tabindex="0"><code>${escape(item.signature)}</code></pre>` : '<p class="warning">This name is exported but its direct definition was not found. The registry should show the name without inventing a signature.</p>'}
      ${item.docstring ? `<p class="docstring">${escape(item.docstring)}</p>` : '<p class="missing-doc">No literal source docstring.</p>'}
    </article>`).join('');
}

function readmeExcerpt(data) {
  if (!data.readme) return '';
  return `<details class="readme-excerpt">
    <summary>README · ${escape(data.readme.section)} <span>${escape(data.readme.path)}:${data.readme.startLine}–${data.readme.endLine}</span></summary>
    <pre tabindex="0"><code>${escape(data.readme.markdown)}</code></pre>
    <p>This is verbatim README content. It is shown as guidance and does not populate the API entries.</p>
  </details>`;
}

export function normalHtml(data, embeddedCss = null) {
  const title = `${data.package.name} API`;
  const styles = embeddedCss ? `<style>${embeddedCss}</style>` : '<link rel="stylesheet" href="/assets/site.css">';
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="text-scale" content="scale=1"><title>${escape(title)} · Sema pkg</title>${styles}</head>
  <body><a class="skip-link" href="#api">Skip to API</a><header class="site-header"><a class="logo" href="/packages/${escape(data.package.name)}">sema<span>pkg</span></a><nav aria-label="Package views"><a href="${apiBase(data)}">API</a><a href="${apiBase(data)}/print">Print / save</a><a href="${apiBase(data)}.md">Markdown</a></nav></header>
  <main><div class="crumbs"><a href="/packages/${escape(data.package.name)}">${escape(data.package.name)}</a><span>/</span><span>${escape(data.package.version)}</span><span>/</span><strong>API</strong></div><section class="intro"><h1>${escape(data.package.name)} <span>API</span></h1><p>${escape(data.package.description)}</p><p class="release-meta">${escape(data.package.version)} · published ${escape(data.provenance.registryPublishedAt)} · <a href="${escape(data.provenance.registryDownloadUrl)}">release tarball</a> · <a href="${escape(data.package.repository)}">source repository</a></p></section>
  <div class="layout"><aside class="toc"><h2>Exported API</h2><ol>${functionList(data)}</ol></aside><div class="content"><section id="api"><div class="section-heading"><h2>API</h2><p>${data.api.length} exports · ${data.api.filter((item) => item.docstring).length} source docstrings</p></div>${apiEntries(data)}${readmeExcerpt(data)}${sourceNotice(data)}</section></div></div></main></body></html>`;
}

export function printHtml(data, css) {
  const title = `${data.package.name} ${data.package.version} API reference`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${escape(title)}</title><style>${css}</style></head><body class="print-page"><main><header class="print-header"><p>sema pkg · offline reference</p><h1>${escape(data.package.name)} <span>${escape(data.package.version)}</span></h1><p>${escape(data.package.description)}</p><dl><div><dt>Release tarball SHA-256</dt><dd>${data.provenance.tarballSha256}</dd></div><div><dt>Entrypoint SHA-256</dt><dd>${data.provenance.sourceSha256}</dd></div></dl></header><section id="api"><h2>Exported API</h2>${apiEntries(data)}</section><footer><p>Generated from the same API document model as the web and Markdown views.</p><p>README narrative is intentionally not converted into function documentation.</p></footer></main></body></html>`;
}

export function markdown(data) {
  const lines = [
    `# ${data.package.name} ${data.package.version} API`,
    '',
    data.package.description,
    '',
    `Release tarball SHA-256: \`${data.provenance.tarballSha256}\``,
    `Entrypoint: \`${data.package.entrypoint}\` (SHA-256 \`${data.provenance.sourceSha256}\`)`,
    '',
    '## Exported API',
    '',
  ];
  for (const item of data.api) {
    lines.push(`### \`${item.name}\``, '');
    if (item.signature) lines.push('```sema', item.signature, '```', '');
    if (item.docstring) lines.push(item.docstring, '');
    if (item.source) lines.push(`Source: [${item.source.path}:${item.source.line}](${sourceFile(data, item.source.path, item.source.line)})`, '');
    if (!item.signature) lines.push('_Exported name; a direct definition was not found. No signature is shown._', '');
  }
  lines.push('## Provenance', '', `- Extraction: ${data.provenance.extraction}`, '- README content is not used as API documentation.');
  return `${lines.join('\n')}\n`;
}

export const cssPath = fileURLToPath(new URL('./site.css', here));
