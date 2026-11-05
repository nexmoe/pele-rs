// The cynefin diagram: what Mermaid makes of a source, as a string.
//
// The diagram is parsed with Mermaid's own Langium parser, and the model is filled by pele's
// port of Mermaid's cynefin database. This module is the shape every summary module of this
// directory has: the summary of the model, the modules of a pele checkout the reference needs,
// what a corpus entry of the type starts with, what the mutator writes into an input, and the
// reference itself.
import { list, loadPele, q } from '../lib.mjs';

export const cynefinSummary = (model) => {
  const out = [];
  out.push(`title:${q(model.title)}|accTitle:${q(model.accTitle)}|accDescr:${q(model.accDescr)}`);
  // The domains keep the order they were read in; a domain that is named twice is one domain
  // and keeps the place it first had.
  for (const [name, domain] of model.domains) {
    out.push(`domain:${q(name)}|items:${list(domain.items.map((item) => item.label))}`);
  }
  for (const transition of model.transitions) {
    out.push(`transition:${q(transition.from)}>${q(transition.to)}|label:${q(transition.label)}`);
  }
  return out.join('\n');
};

export const cynefinModules = {
  '@mermaid-js/parser': ['parse as mermaidParse'],
  'src/diagrams/cynefin/model.ts': ['buildCynefin'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const cynefinKeyword = /(^|\n)\s*cynefin-beta\b/;

export const cynefinFragments = [
  'cynefin-beta',
  'cynefin-beta:',
  'complex',
  'complicated',
  'clear',
  'chaotic',
  'confusion',
  'title ',
  'accTitle: ',
  'accDescr: ',
  'accDescr {',
  '}',
  '-->',
  ':',
  '"',
  "'",
  '\n',
  ' ',
  '\t',
  '%%',
  'é',
  'A',
];

// The bundle of the checkout and Mermaid's own parser, built once.
let bundle;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, cynefinModules);
  return bundle;
}

export const cynefinReference = async (peleDir, src) => {
  const b = await reference(peleDir);
  try {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break to the text it parses.
    const ast = await b.mermaidParse('cynefin', b.encodeEntities(src) + '\n');
    return cynefinSummary(b.buildCynefin(ast, undefined));
  } catch (e) {
    // Mermaid's parser refuses the source; pele-rs refuses it as well.
    const err = new Error(e.message);
    err.code = 'syntax';
    throw err;
  }
};
