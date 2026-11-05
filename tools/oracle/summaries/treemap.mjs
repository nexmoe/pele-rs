// The treemap diagram: what Mermaid makes of a source, as a string.
//
// Mermaid parses a treemap with its own Langium parser, so the model is filled from Mermaid's own
// syntax tree. The tree and the classes are built by pele's port of Mermaid's treemap database,
// and the boxes are laid out by pele's squarified treemap, which is the layout pele-rs draws.
//
// This module is the shape every summary module of this directory has: the summary of the model,
// the modules of a pele checkout the reference needs, what a corpus entry of the type starts
// with, what the mutator writes into an input, and the reference itself.
import { list, loadPele, q } from '../lib.mjs';

// The size the diagram is drawn at: Mermaid sizes it as ten times `nodeWidth` by ten times
// `nodeHeight`, which are 64 and 40 by default. The type reads no configuration.
const WIDTH = 640;
const HEIGHT = 400;
// The space between two boxes, the space a section keeps for its header, and the space it keeps
// around its children on the other three sides.
const GAP = 4;
const HEADER = 28;
const SIDE = 6;

export const treemapSummary = (model) => {
  const out = [];
  out.push(`title:${q(model.title)}|accTitle:${q(model.accTitle)}|accDescr:${q(model.accDescr)}`);
  // The value of an item is its own, or the sum of the values under it. A value that is not a
  // number counts as nothing, as Mermaid's sum reads it.
  const total = (node) => {
    let sum = node.children === undefined ? (node.value ?? 0) : 0;
    for (const child of node.children ?? []) sum += total(child);
    return Number.isNaN(sum) ? 0 : sum;
  };
  const walk = (node, depth) => {
    out.push(
      `node:${depth}|${q(node.name)}|${node.children === undefined ? 'Leaf' : 'Section'}|` +
        `${node.value === undefined ? '-' : String(node.value)}|${String(total(node))}|` +
        `${q(node.classSelector)}|${list(node.cssCompiledStyles ?? [])}`
    );
    for (const child of node.children ?? []) walk(child, depth + 1);
  };
  for (const root of model.roots) walk(root, 0);
  for (const [name, styles] of model.classes) {
    out.push(`class:${q(name)}|${list(styles)}`);
  }
  for (const box of model.boxes ?? []) {
    out.push(`box:${box.series}|${box.x}|${box.y}|${box.w}|${box.h}`);
  }
  return out.join('\n');
};

export const treemapModules = {
  '@mermaid-js/parser': ['parse as mermaidParse'],
  'src/diagrams/treemap/model.ts': ['buildTreemap'],
  'src/diagrams/treemap/layout.ts': ['layoutTreemap'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const treemapKeyword = /(^|\n)\s*treemap(-beta)?\b/;

export const treemapFragments = [
  'treemap',
  'treemap-beta',
  '"',
  "'",
  ':',
  ',',
  ':::',
  'classDef',
  'class',
  'title',
  'accTitle',
  'accDescr',
  '%%',
  '\n',
  ' ',
  'A',
  'é',
  '0',
  '1',
  '100',
  '1,000',
  '0.5',
];

// The bundle of the checkout and Mermaid's own parser, built once.
let bundle;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, treemapModules);
  return bundle;
}

export const treemapReference = async (peleDir, src) => {
  const b = await reference(peleDir);
  try {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break to the text it parses.
    const ast = await b.mermaidParse('treemap', b.encodeEntities(src) + '\n');
    const model = b.buildTreemap(ast, undefined);
    model.boxes = b.layoutTreemap(model.roots, WIDTH, HEIGHT, {
      gap: GAP,
      header: HEADER,
      side: SIDE,
    });
    return treemapSummary(model);
  } catch (e) {
    // Mermaid's parser refuses the source; pele-rs refuses it as well.
    const err = new Error(e.message);
    err.code = 'syntax';
    throw err;
  }
};
