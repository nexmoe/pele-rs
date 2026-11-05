#!/usr/bin/env python3
"""Compare every captured official SVG with Rust. No browser or JS runtime in Rust."""
import gzip
import os
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / 'crates/pele-rs/tests/conformance'
REPORT = Path(tempfile.gettempdir()) / 'pele-rs-conformance-report.json'
GEOMETRY = set('x y x1 y1 x2 y2 cx cy r rx ry width height viewBox d points transform stroke-width font-size dx dy stroke-dasharray stroke-dashoffset'.split())
TOKEN = re.compile(r'[A-Za-z]+|[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?|[^\s,]')

def close(a, b):
    return math.isfinite(a) and math.isfinite(b) and abs(a-b) <= max(.011, 1e-14 * max(abs(a), abs(b)))

def value(a, b, numeric=False):
    if a == b:
        return True
    if not numeric:
        return False
    aa, bb = TOKEN.findall(a), TOKEN.findall(b)
    if len(aa) != len(bb):
        return False
    for x, y in zip(aa, bb):
        if x == y:
            continue
        try:
            if not close(float(x), float(y)):
                return False
        except ValueError:
            return False
    return True

def svg_diff(a, b):
    try:
        aa, bb = list(ET.fromstring(a).iter()), list(ET.fromstring(b).iter())
    except ET.ParseError as e:
        return f'XML: {e}'
    for i, (x, y) in enumerate(zip(aa, bb)):
        if x.tag != y.tag:
            return f'element {i}: {x.tag} != {y.tag}'
        if set(x.attrib) != set(y.attrib):
            return f'element {i} {x.tag}: attributes {dict(x.attrib)} != {dict(y.attrib)}'
        for key in x.attrib:
            if not value(x.attrib[key], y.attrib[key], key in GEOMETRY):
                return f'element {i} @{key}: {x.attrib[key]!r} != {y.attrib[key]!r}'
        if (x.text or '') != (y.text or '') or (x.tail or '') != (y.tail or ''):
            return f'element {i} text: {(x.text, x.tail)!r} != {(y.text, y.tail)!r}'
    if len(aa) != len(bb):
        return f'element count: {len(aa)} != {len(bb)}'

def difference(expected, actual):
    if 'error' in expected:
        if 'error' not in actual:
            return 'official rejected, Rust accepted'
        if expected['error'].get('code') != actual['error'].get('code'):
            return f"error code: {expected['error'].get('code')} != {actual['error'].get('code')}"
        return None
    if 'error' in actual:
        return f"Rust rejected: {actual['error']}"
    x, y = expected['result'], actual['result']
    for key in ('type', 'links'):
        if x[key] != y[key]:
            return f'{key}: {x[key]!r} != {y[key]!r}'
    for key in ('width', 'height'):
        if not close(x[key], y[key]):
            return f'{key}: {x[key]} != {y[key]}'
    return None if x['svg'] == y['svg'] else svg_diff(x['svg'], y['svg'])

def main():
    failures = []
    summary = {}
    selected = sys.argv[1:] or [p.name.split('.')[0] for p in sorted(DATA.glob('*.jsonl.gz')) if p.name != 'units.jsonl.gz']
    for kind in selected:
        raw = gzip.decompress((DATA / f'{kind}.jsonl.gz').read_bytes())
        cases = [json.loads(s) for s in raw.splitlines()]
        proc = subprocess.run([os.environ.get('PELE_RS_CONFORMANCE_BIN', str(ROOT / 'target/release/examples/conformance'))], input=raw, stdout=subprocess.PIPE, check=True)
        actuals = [json.loads(s) for s in proc.stdout.splitlines()]
        assert len(cases) == len(actuals)
        count = 0
        for case, actual in zip(cases, actuals):
            diff = difference(case, actual)
            if diff:
                failures.append({'type': kind, 'id': case['id'], 'difference': diff, 'origins': case['origins'], 'source': case['source'], 'options': case['options']})
                count += 1
        summary[kind] = {'cases': len(cases), 'passed': len(cases)-count, 'failed': count}
        print(kind, summary[kind], flush=True)
        REPORT.write_text(json.dumps({'summary': summary, 'failures': failures}, ensure_ascii=False, indent=2), encoding='utf-8')
    report = {'summary': summary, 'failures': failures}
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(f'Report: {REPORT}')
    return bool(failures)

if __name__ == '__main__':
    sys.exit(main())
