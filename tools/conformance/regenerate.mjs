// Development-only capture of the pinned, unmodified official test suites.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertPeleReference } from '../oracle/lib.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const reference = resolve(process.argv[2] ?? '../pele');
assertPeleReference(reference);
const temporary = mkdtempSync(join(tmpdir(), 'pele-rs-conformance-'));
const results = join(temporary, 'results.json');
try {
  execFileSync(join(reference, 'node_modules/.bin/vitest'), [
    'run', '--config', join(root, 'tools/conformance/vitest.config.mjs'),
    '--reporter=json', `--outputFile=${results}`,
  ], { cwd: reference, env: { ...process.env, TZ: 'UTC', PELE_CAPTURE_DIR: temporary }, stdio: 'inherit' });
  if (!JSON.parse(readFileSync(results, 'utf8')).success) throw new Error('Official suites failed; refusing to generate fixtures.');
  execFileSync(process.execPath, [join(root, 'tools/conformance/fixtures.mjs'), reference, temporary, results], { cwd: root, stdio: 'inherit' });
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
