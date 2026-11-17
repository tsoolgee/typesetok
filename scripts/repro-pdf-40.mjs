// Issue #40 repro: renders the same document with a given tok-cli and reports which font
// each word of the layout is tagged with.
// Usage: node repro-pdf-40.mjs <tokCli> <outPrefix>
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import esbuild from 'esbuild';

const [tokCli, outPrefix] = process.argv.slice(2);
const root = process.cwd();
const outfile = path.join(os.tmpdir(), `repro40-${process.pid}.mjs`);
esbuild.buildSync({
  entryPoints: [path.join(root, 'packages/tok-ui/src/engine/documentBridge.ts')],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  alias: {
    'tok-viewer': path.join(root, 'packages/tok-viewer/src/index.ts'),
    'tok-canvas': path.join(root, 'packages/tok-canvas/src/index.ts'),
    'tok-story-editor': path.join(root, 'packages/tok-story-editor/src/index.ts'),
  },
  outfile,
  logLevel: 'silent',
});
const { documentStateToDocumentRoot } = await import(pathToFileURL(outfile).href);

const quoted = 'וכתב רש״י והר״ן בדף א׳ ע״ב שכך הלכה';
const ascii = 'וכתב רש"י והר"ן בדף א\' ע"ב שכך הלכה';
const plain = 'וכתב רבינו שכך היא ההלכה הפסוקה';
const cases = [
  ['(א) בלי עיצוב: ', quoted, null],
  ['(ב) גופן כמו בייבוא: ', quoted, { fontFamily: 'Frank Ruhl Libre' }],
  ['(ג) מודגש: ', quoted, { fontWeight: 700 }],
  ['(ד) גופן אחר: ', plain, { fontFamily: 'David Libre' }],
  ['(ה) מודגש בלי מירכאות: ', plain, { fontWeight: 700 }],
  ['(ו) מירכאות ASCII עם גופן: ', ascii, { fontFamily: 'Frank Ruhl Libre' }],
];
const paragraphs = cases.map(([label, body, format], i) => {
  const text = label + body;
  const p = { id: `p-${i}`, styleId: 'normal', text };
  if (format) p.spans = [{ start: [...label].length, end: [...text].length, format }];
  return p;
});
const docRoot = documentStateToDocumentRoot({
  title: 'repro 40',
  templateType: 'prose',
  flows: { gemara: paragraphs, rashi: [], tosafot: [], notes: [] },
});

const inFile = `${outPrefix}.json`;
fs.writeFileSync(inFile, JSON.stringify(docRoot, null, 2));
const pdf = spawnSync(tokCli, ['render-pdf', inFile, `${outPrefix}.pdf`], { encoding: 'utf-8' });
if (pdf.status !== 0) throw new Error(`render-pdf failed: ${pdf.stderr || pdf.stdout}`);

const layoutFile = `${outPrefix}-layout.json`;
const ts = spawnSync(tokCli, ['typeset-document', inFile, layoutFile], { encoding: 'utf-8' });
if (ts.status !== 0) throw new Error(`typeset failed: ${ts.stderr || ts.stdout}`);
const pages = JSON.parse(fs.readFileSync(layoutFile, 'utf-8'));
const report = [];
for (const page of pages) {
  for (const frame of page.frames) {
    for (const line of frame.lines) {
      const words = new Map();
      for (const g of line.glyphs) {
        if (!g.character || g.character === ' ') continue;
        const fam = line.fonts?.[g.font_index ?? 0] ?? '?';
        words.set(fam, (words.get(fam) ?? '') + g.character);
      }
      report.push(`${line.paragraph_id}: ${[...words].map(([f, chars]) => `[${f}] ${chars}`).join(' | ')}`);
    }
  }
}
fs.writeFileSync(`${outPrefix}-fonts.txt`, report.join('\n'));
console.log(report.join('\n'));
