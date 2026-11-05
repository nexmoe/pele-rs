// The Ishikawa diagram: what Mermaid makes of a source, as a string.
//
// The diagram is parsed with the parser Mermaid generates from its own grammar, and the model is
// filled by pele's port of Mermaid's Ishikawa database.
import { join, resolve } from 'node:path';
import { asYy, loadPele, q } from '../lib.mjs';

export const ishikawaSummary = (model) => {
  const out = [];
  // The path of a node, as the position of every cause under it.
  const walk = (node, path) => {
    out.push(`node:${path}|${q(node.text)}`);
    for (const [i, child] of (node.children ?? []).entries()) walk(child, `${path}.${i}`);
  };
  if (model.root) walk(model.root, '0');
  out.push(`title:${q(model.title)}`);
  return out.join('\n');
};

export const ishikawaModules = {
  'src/diagrams/ishikawa/db.ts': ['IshikawaDb'],
  'tests/support/jison.ts': ['buildParser'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const ishikawaKeyword = /(^|\n)\s*ishikawa(-beta)?\b/;

export const ishikawaFragments = ['ishikawa', 'ishikawa-beta', 'ishikawa ', '\n', '  ', '    ', ' ', '%%', 'A', 'Process', 'User', 'Equipment', 'LENS', 'SENSOR', 'é', 'Sub cause', 'cause', ':', '-', '#', '123;'];

// The bundle of the checkout and the parser Mermaid's grammar generates, built once.
let bundle;
let parser;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, ishikawaModules);
  parser ??= bundle.buildParser(join(resolve(peleDir), 'tests/compat/ishikawa/upstream/ishikawa.jison'));
  return { bundle, parser };
}

export const ishikawaReference = async (peleDir, src) => {
  const { bundle, parser } = await reference(peleDir);
  const db = new bundle.IshikawaDb();
  parser.yy = asYy(db, {});
  try {
    // Mermaid's Diagram.fromText encodes entities and adds a line break.
    parser.parse(bundle.encodeEntities(src) + '\n');
  } catch (e) {
    // Mermaid's own parser refuses the source; pele-rs refuses it as a syntax error as well.
    throw Object.assign(new Error(e.message), { code: 'syntax' });
  }
  return ishikawaSummary(db);
};
