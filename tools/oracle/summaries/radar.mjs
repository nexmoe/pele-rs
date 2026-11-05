// The radar chart: what Mermaid makes of a source, as a string.
//
// The diagram is parsed with Mermaid's own Langium parser, and the model is filled by pele's
// port of Mermaid's radar database. This module is the shape every summary module of this
// directory has: the summary of the model, the modules of a pele checkout the reference needs,
// what a corpus entry of the type starts with, what the mutator writes into an input, and the
// reference itself.
import { list, loadPele, q } from '../lib.mjs';

export const radarSummary = (model) => {
  const out = [];
  out.push(`axes:${list(model.axes.map((axis) => `${q(axis.name)}|${q(axis.label)}`))}`);
  out.push(
    `curves:${list(
      model.curves.map((curve) => `${q(curve.name)}|${q(curve.label)}|${list(curve.entries.map((value) => String(value)))}`)
    )}`
  );
  out.push(
    `options:showLegend=${model.options.showLegend}|ticks=${model.options.ticks}|max=${model.options.max ?? 'null'}|min=${model.options.min}|graticule=${model.options.graticule}`
  );
  out.push(`title:${q(model.title)}|acc:${q(model.accTitle)}|${q(model.accDescr)}`);
  return out.join('\n');
};

export const radarModules = {
  '@mermaid-js/parser': ['parse as langiumParse'],
  'src/diagrams/radar/model.ts': ['buildRadar'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const radarKeyword = /(^|\n)\s*radar-beta\b/;

export const radarFragments = [
  'radar-beta',
  'radar-beta:',
  'axis',
  'curve',
  'ticks',
  'min',
  'max',
  'showLegend',
  'graticule',
  'circle',
  'polygon',
  'true',
  'false',
  '{',
  '}',
  '[',
  ']',
  '(',
  ')',
  ',',
  ':',
  '"',
  "'",
  'A',
  'B',
  'C',
  '1',
  '2',
  '10',
  '1.5',
  '0',
  '\n',
  ' ',
  '\t',
  '%%',
  'é',
];

// The bundle of the checkout, built once.
let bundle;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, radarModules);
  return bundle;
}

export const radarReference = async (peleDir, src) => {
  const pele = await reference(peleDir);
  let ast;
  try {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break to the text it parses.
    ast = await pele.langiumParse('radar', pele.encodeEntities(src) + '\n');
  } catch (e) {
    // Mermaid refuses the text in its grammar; pele-rs calls that a syntax error.
    const error = new Error(String(e.message ?? e));
    error.code = 'syntax';
    throw error;
  }
  return radarSummary(pele.buildRadar(ast, undefined));
};
