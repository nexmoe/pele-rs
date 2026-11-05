// Compact actual render calls captured while running the pinned official Vitest suites.
import { readFileSync, readdirSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { loadPele, assertPeleReference } from '../oracle/lib.mjs';

const [reference, captureDir, resultsFile] = process.argv.slice(2);
if (!resultsFile) throw new Error('usage: fixtures.mjs <pele> <capture-dir> <vitest-results.json>');
const pin = assertPeleReference(reference);
const { metricsMeasurer, detectType } = await loadPele(reference, {
  'src/text/measurer.ts': ['metricsMeasurer'], 'src/index.ts': ['detectType'],
});
const target = resolve('crates/pele-rs/tests/conformance');
mkdirSync(target, { recursive: true });
const groups = new Map(), excluded = [], ids = new Map();
let calls = 0;
for (const file of readdirSync(captureDir).filter(f => f.endsWith('.jsonl') && !f.endsWith('.units.jsonl')).sort()) {
  for (const line of readFileSync(join(captureDir, file), 'utf8').trim().split('\n')) {
    const entry = JSON.parse(line);
    calls++;
    // Rust strings are Unicode scalar values. Preserve an explicit inventory of JS-only inputs.
    if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(JSON.stringify(entry).replace(/\\u([\da-f]{4})/gi, (_, h) => String.fromCharCode(parseInt(h, 16))))) {
      excluded.push({ file: entry.file, test: entry.test, reason: 'lone UTF-16 surrogate', source: entry.source });
      continue;
    }
    const reason=entry.unregistered ? 'JS module registry before registration (Rust uses compile-time features)' : entry.foreignException ? 'JavaScript callback exception (Rust callbacks use panic unwinding)' : (JSON.stringify(entry.options?.config) ?? '').includes('"$number":"NaN"') ? 'NaN in a JavaScript config object (not representable in Rust JSON configuration)' : undefined;
    if (reason) {excluded.push({file:entry.file,test:entry.test,reason,source:entry.source,options:entry.options,error:entry.error});continue;}
    delete entry.unregistered;delete entry.foreignException;
    const type = entry.result?.type ?? (() => { try { return detectType(entry.source); } catch { return null; } })() ?? 'unknown';
    if (type !== 'gantt') delete entry.options.now;
    if (!entry.widths.length || entry.widths.every(([text, size, style, width]) => Math.abs(metricsMeasurer.width(text, size, style) - width) < 1e-10)) {
      delete entry.widths; delete entry.customMeasurer;
    }
    const origin = `${entry.file}: ${entry.test}`;
    delete entry.file; delete entry.test;
    const key = createHash('sha256').update(JSON.stringify(entry)).digest('hex');
    const existing = ids.get(key);
    if (existing) { if (!existing.origins.includes(origin)) existing.origins.push(origin); continue; }
    const fixture = { id: key.slice(0, 16), origins: [origin], ...entry };
    ids.set(key, fixture);
    if (!groups.has(type)) groups.set(type, []);
    groups.get(type).push(fixture);
  }
}
const counts = {};
for (const [type, entries] of [...groups].sort(([a], [b]) => a.localeCompare(b))) {
  entries.sort((a,b) => a.id.localeCompare(b.id));
  writeFileSync(join(target, `${type}.jsonl.gz`), gzipSync(entries.map(e => JSON.stringify(e)).join('\n') + '\n', { level: 9 }));
  counts[type] = { cases: entries.length, rendered: entries.filter(e => e.result).length };
}
const units=new Map();
for (const file of readdirSync(captureDir).filter(f=>f.endsWith('.units.jsonl')).sort()) {
  for (const line of readFileSync(join(captureDir,file),'utf8').trim().split('\n')) {
    const entry=JSON.parse(line);
    if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(line.replace(/\\u([\da-f]{4})/gi,(_,h)=>String.fromCharCode(parseInt(h,16))))) {excluded.push({origins:entry.origins,reason:'lone UTF-16 surrogate in a unit call',name:entry.name});continue;}
    const origins=entry.origins;delete entry.origins;
    const id=createHash('sha256').update(JSON.stringify(entry)).digest('hex').slice(0,16);
    if (units.has(id)) {for(const origin of origins)if(!units.get(id).origins.includes(origin))units.get(id).origins.push(origin);}
    else units.set(id,{id,origins,...entry});
  }
}
writeFileSync(join(target,'units.jsonl.gz'),gzipSync([...units.values()].sort((a,b)=>a.id.localeCompare(b.id)).map(e=>JSON.stringify(e)).join('\n')+'\n',{level:9}));
const results = JSON.parse(readFileSync(resultsFile, 'utf8'));
const tests = results.testResults.map(file => ({
  file: file.name.replace(/^.*\/pele\//, ''),
  tests: file.assertionResults.map(t => ({ name: t.fullName, status: t.status })),
}));
writeFileSync(join(target, 'inventory.json'), JSON.stringify({ reference: pin, calls, cases: ids.size, units: units.size, counts, tests, excluded }, null, 2) + '\n');
const hashes = Object.fromEntries(readdirSync(target).filter(f => f.endsWith('.jsonl.gz')).sort().map(f => [f, createHash('sha256').update(readFileSync(join(target, f))).digest('hex')]));
writeFileSync(join(target, 'manifest.json'), JSON.stringify({ reference: pin, calls, cases: ids.size, units: units.size, counts, sha256: hashes }, null, 2) + '\n');
console.log(JSON.stringify({ calls, cases: ids.size, units: units.size, counts, excluded: excluded.length }, null, 2));
