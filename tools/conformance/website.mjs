// Capture the official website's examples without mounting a browser or host app.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { gzipSync } from 'node:zlib';
import { loadPele, assertPeleReference } from '../oracle/lib.mjs';

const pele = process.argv[2] ?? '../pele';
const pin = assertPeleReference(pele);
const root = new URL('../../crates/pele-rs/tests/website/', import.meta.url);
const markdown = readFileSync(new URL('examples.md', root), 'utf8');
const { render } = await loadPele(pele, { 'src/index.ts': ['render'] });
const examples = [...markdown.matchAll(/```mermaid\n([\s\S]*?)\n```/g)];
if (examples.length !== 58) throw new Error(`Expected 58 examples, found ${examples.length}`);
const records = [];
for (const [index, match] of examples.entries()) {
  const title = [...markdown.slice(0, match.index).matchAll(/^## (.+)$/gm)].at(-1)[1];
  for (const width of [undefined, 320, 640, 1024]) {
    const options = { now: 0, idPrefix: `website-${index + 1}`, ...(width === undefined ? {} : { maxWidth: width }) };
    const result = render(match[1], options);
    records.push({ title, index: index + 1, source: match[1], options, result });
  }
}
writeFileSync(new URL('renders.jsonl.gz', root), gzipSync(records.map(r => JSON.stringify(r)).join('\n') + '\n', { mtime: 0 }));
writeFileSync(new URL('manifest.json', root), JSON.stringify({
  source: 'https://pele.run/examples.md', retrieved: '2026-11-05', pele: pin,
  markdownSha256: createHash('sha256').update(markdown).digest('hex'),
  examples: examples.length, types: new Set(records.map(r => r.result.type)).size, renders: records.length,
}, null, 2) + '\n');
console.log(`${examples.length} official website examples, ${records.length} render references`);
