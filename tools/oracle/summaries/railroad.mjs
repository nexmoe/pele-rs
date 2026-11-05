// The railroad diagram: what Mermaid makes of a source, as a string.
//
// Mermaid has four grammars for this diagram — its own notation, EBNF, ABNF and PEG — and
// each has its own parser in `packages/mermaid/src/diagrams/railroad/parser/`. Every one of
// them reads the text with Mermaid's own Langium parser and then turns the tree into the
// same model: a rule is a name and a tree of nodes.
//
// This module reads the text with Mermaid's own Langium parser and turns the tree into the
// model the way Mermaid's four parsers do, so that the reference is Mermaid's behaviour and
// not a port of it.

import { list, loadPele, q } from '../lib.mjs';

// -- Mermaid's own notation, from `parser/railroadParser.ts`. --

const irExpression = (expr) => {
  switch (expr.$type) {
    case 'RailroadTerminalExpr':
      return { type: 'terminal', value: expr.value };
    case 'RailroadNonTerminalExpr':
      return { type: 'nonterminal', name: expr.name };
    case 'RailroadSpecialExpr':
      return { type: 'special', text: expr.text };
    case 'RailroadSequenceExpr': {
      const elements = expr.elements.map(irExpression);
      return elements.length === 1 ? elements[0] : { type: 'sequence', elements };
    }
    case 'RailroadChoiceExpr': {
      const alternatives = expr.alternatives.map(irExpression);
      return alternatives.length === 1 ? alternatives[0] : { type: 'choice', alternatives };
    }
    case 'RailroadOptionalExpr':
      return { type: 'optional', element: irExpression(expr.element) };
    case 'RailroadOneOrMoreExpr':
      return { type: 'repetition', element: irExpression(expr.element), min: 1, max: Infinity };
    case 'RailroadZeroOrMoreExpr':
      return { type: 'repetition', element: irExpression(expr.element), min: 0, max: Infinity };
    default:
      throw new Error(`Unsupported railroad expression: ${expr.$type}`);
  }
};

const irRules = (ast) => ast.rules.map((rule) => ({ name: rule.name, definition: irExpression(rule.definition) }));

// -- EBNF, from `parser/ebnfParser.ts`. --

const ebnfChoice = (choice) => {
  const alternatives = choice.alternatives.map(ebnfSequence);
  if (alternatives.length === 1) {
    return alternatives[0];
  }
  return { type: 'choice', alternatives };
};

const ebnfSequence = (sequence) => {
  const elements = sequence.elements.map(ebnfTerm);
  if (elements.length === 1) {
    return elements[0];
  }
  return { type: 'sequence', elements };
};

const ebnfPrimary = (primary) => {
  switch (primary.$type) {
    case 'EbnfTerminal':
      return { type: 'terminal', value: primary.value };
    case 'EbnfNonTerminal':
      return { type: 'nonterminal', name: primary.name };
    case 'EbnfSpecial':
      return { type: 'special', text: primary.text };
    case 'EbnfGroup':
      return ebnfChoice(primary.element);
    case 'EbnfOptional':
      return { type: 'optional', element: ebnfChoice(primary.element) };
    case 'EbnfRepetition':
      return { type: 'repetition', element: ebnfChoice(primary.element), min: 0, max: Infinity };
    default:
      throw new Error(`Unsupported EBNF primary node: ${primary.$type}`);
  }
};

const ebnfPostfix = (node, postfix) => {
  switch (postfix.$type) {
    case 'EbnfOptionalPostfix':
      return { type: 'optional', element: node };
    case 'EbnfZeroOrMorePostfix':
      return { type: 'repetition', element: node, min: 0, max: Infinity };
    case 'EbnfOneOrMorePostfix':
      return { type: 'repetition', element: node, min: 1, max: Infinity };
    case 'EbnfExceptionPostfix':
      return {
        type: 'sequence',
        elements: [node, { type: 'terminal', value: '-' }, ebnfPrimary(postfix.except)],
      };
    default:
      throw new Error(`Unsupported EBNF postfix node: ${postfix.$type}`);
  }
};

const ebnfTerm = (term) =>
  term.postfixes.reduce((node, postfix) => ebnfPostfix(node, postfix), ebnfPrimary(term.base));

const ebnfRules = (ast) =>
  ast.rules.map((rule) => ({ name: rule.name, definition: ebnfChoice(rule.definition) }));

// -- ABNF, from `parser/abnfParser.ts`. --

const abnfAlternation = (alt) => {
  const alternatives = alt.alternatives.map(abnfConcatenation);
  if (alternatives.length === 1) {
    return alternatives[0];
  }
  return { type: 'choice', alternatives };
};

const abnfConcatenation = (concat) => {
  const elements = concat.elements.map(abnfElement);
  if (elements.length === 1) {
    return elements[0];
  }
  return { type: 'sequence', elements };
};

const abnfRepeat = (repeat) => {
  if (repeat.includes('*')) {
    const [minStr, maxStr] = repeat.split('*');
    const min = minStr ? parseInt(minStr, 10) : 0;
    const max = maxStr ? parseInt(maxStr, 10) : Infinity;
    return { min, max };
  }
  const exact = parseInt(repeat, 10);
  return { min: exact, max: exact };
};

const abnfElement = (element) => {
  const inner = abnfPrimary(element.primary);
  if (!element.repeat) {
    return inner;
  }
  const { min, max } = abnfRepeat(element.repeat);
  if (min === 0 && max === 1) {
    return { type: 'optional', element: inner };
  }
  return { type: 'repetition', element: inner, min, max };
};

const abnfPrimary = (primary) => {
  switch (primary.$type) {
    case 'AbnfStringLiteral':
      return { type: 'terminal', value: primary.value };
    case 'AbnfNumVal':
      return { type: 'terminal', value: primary.value };
    case 'AbnfRuleName':
      return { type: 'nonterminal', name: primary.name };
    case 'AbnfGroup':
      return abnfAlternation(primary.element);
    case 'AbnfOptionalGroup':
      return { type: 'optional', element: abnfAlternation(primary.element) };
    default:
      throw new Error(`Unsupported ABNF primary node: ${primary.$type}`);
  }
};

const abnfRules = (ast) =>
  ast.rules.map((rule) => ({ name: rule.name, definition: abnfAlternation(rule.definition) }));

// -- PEG, from `parser/pegParser.ts`. --

const pegChoice = (choice) => {
  const alternatives = choice.alternatives.map(pegSequence);
  if (alternatives.length === 1) {
    return alternatives[0];
  }
  return { type: 'choice', alternatives };
};

const pegSequence = (sequence) => {
  const elements = sequence.elements.map(pegPrefix);
  if (elements.length === 1) {
    return elements[0];
  }
  return { type: 'sequence', elements };
};

const pegLabel = (node) => {
  switch (node.type) {
    case 'terminal':
      return `"${node.value}"`;
    case 'nonterminal':
      return node.name;
    case 'special':
      return node.text;
    default:
      return '(...)';
  }
};

const pegPrefix = (prefix) => {
  const inner = pegSuffix(prefix.suffix);
  if (!prefix.operator) {
    return inner;
  }
  const label = prefix.operator === '&' ? `&${pegLabel(inner)}` : `!${pegLabel(inner)}`;
  return { type: 'special', text: label };
};

const pegSuffix = (suffix) => {
  const inner = pegPrimary(suffix.primary);
  if (!suffix.operator) {
    return inner;
  }
  switch (suffix.operator) {
    case '?':
      return { type: 'optional', element: inner };
    case '*':
      return { type: 'repetition', element: inner, min: 0, max: Infinity };
    case '+':
      return { type: 'repetition', element: inner, min: 1, max: Infinity };
    default:
      throw new Error(`Unsupported PEG suffix operator: ${suffix.operator}`);
  }
};

const pegPrimary = (primary) => {
  switch (primary.$type) {
    case 'PegLiteral':
      return { type: 'terminal', value: primary.value };
    case 'PegIdentifier':
      return { type: 'nonterminal', name: primary.name };
    case 'PegGroup':
      return pegChoice(primary.element);
    case 'PegAny':
      return { type: 'special', text: primary.dot };
    default:
      throw new Error(`Unsupported PEG primary node: ${primary.$type}`);
  }
};

const pegRules = (ast) =>
  ast.rules.map((rule) => ({ name: rule.name, definition: pegChoice(rule.definition) }));

// -- The model, and the way it is written. --

export const railroadSummary = (model) => {
  const out = [];
  out.push(
    `title:${q(model.title)}|accTitle:${q(model.accTitle)}|accDescr:${q(model.accDescr)}`
  );
  for (const rule of model.rules) {
    out.push(`rule:${q(rule.name)}|${railroadNode(rule.definition)}`);
  }
  return out.join('\n');
};

const railroadNode = (node) => {
  switch (node.type) {
    case 'terminal':
      return `t(${q(node.value)})`;
    case 'nonterminal':
      return `n(${q(node.name)})`;
    case 'special':
      return `s(${q(node.text)})`;
    case 'sequence':
      return `seq(${node.elements.map(railroadNode).join(',')})`;
    case 'choice':
      return `alt(${node.alternatives.map(railroadNode).join(',')})`;
    case 'optional':
      return `opt(${railroadNode(node.element)})`;
    case 'repetition':
      return `rep(${node.min},${node.max === Infinity ? 'inf' : node.max},${railroadNode(node.element)})`;
    default:
      throw new Error(`Unsupported railroad node: ${node.type}`);
  }
};

export const railroadModules = {
  '@mermaid-js/parser': ['parse as mermaidParse'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const railroadKeyword = /(^|\n)\s*railroad-(ebnf-|abnf-|peg-)?beta/i;

export const railroadFragments = [
  'railroad-beta',
  'railroad-ebnf-beta',
  'railroad-abnf-beta',
  'railroad-peg-beta',
  'rule',
  '=',
  '::=',
  '<-',
  ';',
  '(',
  ')',
  '[',
  ']',
  '{',
  '}',
  ',',
  '|',
  '/',
  '&',
  '!',
  '?',
  '*',
  '+',
  '-',
  '.',
  'terminal',
  'nonterminal',
  'special',
  'sequence',
  'choice',
  'optional',
  'oneOrMore',
  'zeroOrMore',
  '"a"',
  "'a'",
  'a',
  'title',
  'accTitle',
  'accDescr',
  '%%',
  '\n',
  ' ',
];

// The bundle of the checkout and Mermaid's own parser, built once.
let bundle;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, railroadModules);
  return bundle;
}

// Which of Mermaid's four grammars the text is written in. Mermaid has a detector per
// keyword, each matching without regard to case.
const notationOf = (src) => {
  const m = /^\s*railroad-(ebnf-|abnf-|peg-)?beta/i.exec(src);
  if (m === null) {
    return undefined;
  }
  const infix = (m[1] ?? '').toLowerCase();
  if (infix === 'ebnf-') {
    return { type: 'railroadEbnf', rules: ebnfRules };
  }
  if (infix === 'abnf-') {
    return { type: 'railroadAbnf', rules: abnfRules };
  }
  if (infix === 'peg-') {
    return { type: 'railroadPeg', rules: pegRules };
  }
  return { type: 'railroad', rules: irRules };
};

export const railroadReference = async (peleDir, src) => {
  const b = await reference(peleDir);
  const notation = notationOf(src);
  if (notation === undefined) {
    const err = new Error('No railroad diagram keyword found.');
    err.code = 'syntax';
    throw err;
  }
  try {
    // Mermaid's Diagram.fromText encodes the entities of the whole diagram before any parser
    // sees it, and adds a line break to the text it parses.
    const ast = await b.mermaidParse(notation.type, b.encodeEntities(src) + '\n');
    return railroadSummary({
      title: ast.title || undefined,
      accTitle: ast.accTitle || undefined,
      accDescr: ast.accDescr || undefined,
      rules: notation.rules(ast),
    });
  } catch (e) {
    // Mermaid's parser refuses the source; pele-rs refuses it as well.
    const err = new Error(e.message);
    err.code = 'syntax';
    throw err;
  }
};
