#!/usr/bin/env node
// Compare the fixture generator's document with sema-reader's CLI AST output.
import { readFile } from 'node:fs/promises';

const root = new URL('.', import.meta.url);
const [ast, document] = await Promise.all([
  readFile(new URL('./fixture-release/sema-ast.json', root), 'utf8').then(JSON.parse),
  readFile(new URL('./generated/api.json', root), 'utf8').then(JSON.parse),
]);
const symbol = (node) => node?.type === 'symbol' ? node.value : null;
function render(node) {
  if (node.type === 'list') return `(${node.children.map(render).join(' ')})`;
  return node.value;
}
if (symbol(ast.children?.[0]) !== 'module') throw new Error('sema ast did not produce a module root');
const exportForm = ast.children.find((node) => symbol(node.children?.[0]) === 'export');
const astExports = exportForm?.children.slice(1).map(symbol);
const documentExports = document.api.map((entry) => entry.name);
if (JSON.stringify(astExports) !== JSON.stringify(documentExports)) {
  throw new Error('fixture export list differs from sema ast output');
}
const astDefinitions = new Map(ast.children
  .filter((node) => symbol(node.children?.[0]) === 'define' && node.children[1]?.type === 'list')
  .map((node) => [symbol(node.children[1].children[0]), {
    signature: render(node.children[1]),
    docstring: node.children[2]?.type === 'string' ? node.children[2].value : null,
  }]));
for (const entry of document.api) {
  const definition = astDefinitions.get(entry.name);
  if (!definition) throw new Error(`${entry.name} is not a direct definition in sema ast`);
  if (definition.signature !== entry.signature) throw new Error(`${entry.name} signature differs from sema ast`);
  if (definition.docstring !== entry.docstring) throw new Error(`${entry.name} docstring differs from sema ast`);
}
console.log(`sema ast independently confirms ${documentExports.length} exports, signatures, and literal docstrings`);
