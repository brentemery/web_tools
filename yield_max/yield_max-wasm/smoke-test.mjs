// Functional smoke test for the committed wasm: loads pkg/ exactly as the
// page does and checks it computes the known answer. This is the property
// that matters -- that the shipped binary works -- which a byte-for-byte
// comparison never actually verified.
import { readFile } from 'node:fs/promises';
import init, {
  analyze_wafer, mask_rows, mask_sites, legend, tie_breaks, grades_best_first,
  row_labels, col_labels,
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

// The report is labeled, and its labels are read back on the round-trip below.
if (!r.report.includes('\nO ')) fail('the marked grid must carry row labels');
if (!r.report.includes('# 12345678901234567')) fail('the report must number its columns');

// The report must round-trip: our own output is valid input.
const report = r.report;
b.free(); r.free();
const r2 = analyze_wafer(report);
if (r2.report !== report) fail('round-trip: re-running changed the report');
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
