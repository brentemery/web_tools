// Functional smoke test for the committed wasm: loads pkg/ exactly as the
// page does and checks it computes the known answer. This is the property
// that matters -- that the shipped binary works -- which a byte-for-byte
// comparison never actually verified.
import { readFile } from 'node:fs/promises';
import init, {
  analyze_wafer, mask_rows, mask_sites, legend, tie_breaks, grades_best_first,
  row_labels, col_labels, board_size, max_input_bytes, mask_covers, glyph,
} from './pkg/yield_max_wasm.js';

const wasm = await readFile(new URL('./pkg/yield_max_wasm_bg.wasm', import.meta.url));
await init({ module_or_path: wasm });

const fixture = (name) =>
  readFile(new URL(`../testdata/${name}`, import.meta.url), 'utf8');

let failed = 0;
const check = (name, got, want) => {
  if (got !== want) { console.error(`  FAIL ${name}: got ${got}, want ${want}`); failed++; }
};
const fail = (msg) => { console.error(`  FAIL ${msg}`); failed++; };

// --- The ungraded sample: its version-2 answer must be unchanged ------------
const map = await readFile(new URL('../test_wafer.txt', import.meta.url), 'utf8');
// Called with one argument, as the original API was, to keep that path tested.
const r = analyze_wafer(map);
const b = r.best;

for (const [name, got, want] of [
  ['row', b.row, 2],
  ['col', b.col, 4],
  ['label', b.label, 'C5'],
  ['center_row', b.center_row, 7],
  ['center_col', b.center_col, 9],
  ['center_label', b.center_label, 'H10'],
  ['good', b.good, 57],
  ['good4', b.good4, 0],
  ['good1', b.good1, 57],
  ['defect', b.defect, 36],
  ['overhang', b.overhang, 0],
  ['sites', b.sites, 93],
  ['tiebreak', r.tiebreak, 'grade'],
  ['mask_sites()', mask_sites(), 93],
  ['mask_rows() length', mask_rows().length, 11],
  ['tie_breaks()', tie_breaks().join(','), 'grade,total'],
  ['grades_best_first()', [...grades_best_first()].join(','), '4,3,2,1'],
  // The row letters skip 'N' on purpose; the UI draws its axis from this list.
  ['row_labels()', row_labels().join(''), 'ABCDEFGHIJKLMOPQR'],
  ['col_labels() first/last', `${col_labels()[0]}-${col_labels()[16]}`, '1-17'],
]) check(name, got, want);

if (!legend().includes('D=good4')) fail('legend() must describe the graded alphabet');

// --- The values the page reads instead of restating ------------------------
check('board_size()', board_size(), 17);
check('max_input_bytes()', max_input_bytes(), 64 * 1024);
for (const [args, want] of [
  [['good', 4, false], '4'], [['good', 4, true], 'D'],
  [['good', 1, false], '1'], [['good', 1, true], 'A'],
  [['defect', 0, false], 'X'], [['defect', 0, true], '*'],
  [['absent', 0, false], '.'], [['absent', 0, true], '-'],
]) check(`glyph(${args.join(', ')})`, glyph(...args), want);
for (const bad of [['good', 5, false], ['sideways', 0, false]]) {
  try { glyph(...bad); fail(`glyph(${bad.join(', ')}) was accepted`); } catch { /* expected */ }
}
// The page draws the region outline with mask_covers(); every placement must
// cover exactly the mask's sites.
{
  let covered = 0;
  for (let r = 0; r < 17; r++) for (let c = 0; c < 17; c++) if (mask_covers(2, 4, r, c)) covered++;
  check('mask_covers() site count', covered, 93);
  check('mask_covers() center', mask_covers(2, 4, 7, 9), true);
  check('mask_covers() top-left corner is not a site', mask_covers(2, 4, 2, 4), false);
}

// The report is labeled, and its labels are read back on the round-trip below.
if (!r.report.includes('\nO ')) fail('the marked grid must carry row labels');
if (!r.report.includes('# 12345678901234567')) fail('the report must number its columns');

// The report must round-trip: our own output is valid input.
const report = r.report;
b.free(); r.free();
const r2 = analyze_wafer(report);
if (r2.report !== report) fail('round-trip: re-running changed the report');
// Its marks are exactly the result, so nothing is being replaced.
check('round-trip warning', r2.warning, '');
r2.best.free(); r2.free();

// --- Graded wafers: grade 4 is what gets maximized --------------------------
const graded = await fixture('grades_mixed.txt');
const g = analyze_wafer(graded);
const gb = g.best;
for (const [name, got, want] of [
  ['grades_mixed row', gb.row, 3],
  ['grades_mixed col', gb.col, 4],
  ['grades_mixed good4', gb.good4, 21],
  ['grades_mixed good3', gb.good3, 18],
  ['grades_mixed good2', gb.good2, 11],
  ['grades_mixed good1', gb.good1, 20],
  ['grades_mixed good total', gb.good, 70],
]) check(name, got, want);
// `good` must be the sum of the grades, not a subset of them.
check('good == sum of grades', gb.good, gb.good1 + gb.good2 + gb.good3 + gb.good4);
gb.free(); g.free();

// --- The tie-break option actually changes the answer ----------------------
const div = await fixture('tiebreak_divergent.txt');
for (const [policy, row, col, good] of [
  [undefined, 2, 2, 64],   // default
  ['', 2, 2, 64],          // explicitly "no opinion"
  ['grade', 2, 2, 64],
  ['total', 4, 2, 68],
]) {
  const a = analyze_wafer(div, policy);
  const p = a.best;
  const label = `tiebreak=${policy === undefined ? 'default' : `'${policy}'`}`;
  check(`${label} row`, p.row, row);
  check(`${label} col`, p.col, col);
  check(`${label} good`, p.good, good);
  // Both policies maximize grade 4; only the tie is settled differently.
  check(`${label} good4`, p.good4, 17);
  if (policy) check(`${label} reported`, a.tiebreak, policy);
  p.free(); a.free();
}

// --- A report's recorded tie-break is honoured, as the CLI honours it ------
// Re-analyzing a report with no policy named must reproduce it under the
// policy its header recorded, not quietly switch to the default.
{
  const made = analyze_wafer(div, 'total');
  const totalReport = made.report;
  made.best.free(); made.free();

  const again = analyze_wafer(totalReport);
  check('header policy reused', again.tiebreak, 'total');
  check('header policy source', again.tiebreak_source, 'header');
  check('header policy reproduces the report', again.report, totalReport);
  again.best.free(); again.free();

  // Naming the recorded policy is fine; contradicting it must throw.
  const agree = analyze_wafer(totalReport, 'total');
  check('agreeing policy source', agree.tiebreak_source, 'requested');
  agree.best.free(); agree.free();
  try {
    analyze_wafer(totalReport, 'grade');
    fail('a policy contradicting the report header was accepted');
  } catch (e) {
    if (!String(e).includes("'total'")) fail(`contradiction error must name the recorded policy: ${e}`);
  }

  const plain = analyze_wafer(div);
  check('no header, nothing asked: source', plain.tiebreak_source, 'default');
  plain.best.free(); plain.free();
}

// --- Marks at a legal region that is not the result are replaced, loudly ---
{
  // An all-good wafer with the region hand-marked at (2, 2): legal, but the
  // solver prefers an earlier placement.
  const mask = mask_rows();
  const rows = Array.from({ length: 17 }, (_, r) =>
    Array.from({ length: 17 }, (_, c) => {
      const dr = r - 2, dc = c - 2;
      const inside = dr >= 0 && dr < 11 && dc >= 0 && dc < 11 && mask[dr][dc] === 'O';
      return inside ? 'A' : '1';
    }).join(''));
  const a = analyze_wafer(rows.join('\n'));
  if (!(a.best.row !== 2 || a.best.col !== 2)) fail('precondition: (2, 2) must not win');
  if (!a.warning.includes('not this run\'s result')) fail(`legal non-winning marks: got warning ${JSON.stringify(a.warning)}`);
  if (!a.warning.includes('centered on H8')) fail(`warning must name the marked region: ${a.warning}`);
  a.best.free(); a.free();
}

// An unrecognized policy must throw, not quietly use the default.
try { analyze_wafer(map, 'sideways'); fail('an unknown tiebreak was accepted'); }
catch { /* expected */ }

// --- A version-2 report (in-region good die spelled `Z`) still parses ------
const legacy = await fixture('legacy_z_roundtrip.txt');
if (!legacy.includes('Z')) fail('legacy fixture should contain the v2 glyph');
const l = analyze_wafer(legacy);
check('legacy row', l.best.row, 2);
check('legacy good', l.best.good, 57);
if (l.report.includes('Z')) fail("v2's Z must not be emitted");
if (!l.report.includes('A')) fail('v3 must mark in-region grade-1 die as A');
check('legacy center', l.best.center_label, 'H10');
l.best.free(); l.free();

// --- Rejections ------------------------------------------------------------
// Malformed input must reject, not silently produce something.
try { analyze_wafer('garbage'); fail('malformed input was accepted'); }
catch { /* expected */ }

// A row label that disagrees with its position means a row was inserted or
// dropped; it must reject rather than be stripped and trusted positionally.
try {
  analyze_wafer(map.trimEnd().split('\n').slice(-17)
    .map((line, i) => `${i === 5 ? 'Z' : 'ABCDEFGHIJKLMOPQR'[i]} ${line}`).join('\n'));
  fail('a mislabeled row was accepted');
} catch { /* expected */ }

// A wafer with no die anywhere has no legal 200mm placement.
try {
  analyze_wafer(Array(17).fill('.'.repeat(17)).join('\n'));
  fail('an all-absent wafer was accepted');
} catch { /* expected */ }

if (failed) { console.error(`${failed} check(s) failed`); process.exit(1); }
console.log('  committed wasm loads and computes the expected result');
