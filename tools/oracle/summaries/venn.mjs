// The venn diagram: what Mermaid makes of a source, as a string.
//
// The diagram is parsed with the parser Mermaid generates from its own grammar, and the model is
// filled by pele's port of Mermaid's venn database. This module is the shape every summary
// module of this directory has: the summary of the model, the modules of a pele checkout the
// reference needs, what a corpus entry of the type starts with, what the mutator writes into an
// input, and the reference itself.
import { join, resolve } from 'node:path';
import { asYy, list, loadPele, q } from '../lib.mjs';

export const vennSummary = (model) => {
  const out = [];
  out.push(`title:${q(model.title)}`);
  for (const subset of model.subsets) {
    out.push(`subset:${list(subset.sets)}|size=${subset.size}|label=${q(subset.label)}`);
  }
  for (const text of model.textNodes) {
    out.push(`text:${list(text.sets)}|id=${q(text.id)}|label=${q(text.label)}`);
  }
  for (const entry of model.styles) {
    // The properties, in the order they were declared.
    const props = [...entry.styles].map(([key, value]) => `${key}=${q(value)}`);
    out.push(`style:${list(entry.targets)}|${props.join(',')}`);
  }
  return out.join('\n');
};

export const vennModules = {
  'src/diagrams/venn/db.ts': ['VennDb'],
  'tests/support/jison.ts': ['buildParser'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const vennKeyword = /(^|\n)\s*venn-beta\b/;

export const vennFragments = [
  'venn-beta',
  'venn-beta:',
  'title ',
  'set ',
  'union ',
  'text ',
  'style ',
  'A',
  'B',
  'C',
  'A["Alpha"]',
  '[Alpha]',
  '["Alpha"]',
  'A,B',
  'A,B,C',
  ':20',
  ':12',
  ':3',
  ':5.3',
  'fill:#ff6b6b',
  'color:#333',
  'fill:rgb(255, 0, 128)',
  'fill:rgba(255, 0, 128, 0.5)',
  ',',
  ':',
  '"',
  "'",
  '[',
  ']',
  '1',
  '-1',
  '1.5',
  '.5',
  '\n',
  ' ',
  '\t',
  '%%',
  '---\n',
  'é',
];

// The bundle of the checkout and the parser Mermaid's grammar generates, built once.
let bundle;
let parser;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, vennModules);
  parser ??= bundle.buildParser(join(resolve(peleDir), 'tests/compat/venn/upstream/venn.jison'));
  return { bundle, parser };
}

export const vennReference = async (peleDir, src) => {
  const { bundle, parser } = await reference(peleDir);
  const db = new bundle.VennDb();
  parser.yy = asYy(db);
  try {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break.
    parser.parse(bundle.encodeEntities(src) + '\n');
  } catch (e) {
    // Mermaid refuses the text in its grammar; pele-rs calls that a syntax error. A union of
    // one identifier is the exception: the text follows the grammar but says something invalid,
    // which is a semantic error.
    const message = String(e.message ?? e);
    const error = new Error(message);
    error.code = e.code ?? (message.startsWith('union requires multiple identifiers') ? 'semantic' : 'syntax');
    throw error;
  }
  return vennSummary(db);
};
